import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import {
  createAssistantMessageEventStream,
  getCurrentTools,
  withoutInitialSystemMessage,
} from '@earendil-works/pi-ai';
import type {
  Api,
  AssistantMessage,
  JsonObject,
  Model,
  Provider,
  ToolCall,
} from '@earendil-works/pi-ai';
import { ConversationProvider } from './support/conversation/conversation-provider.ts';
import type { AgentContext } from '@zf/machines';
import { CommandTool } from '@zf/capabilities';
import { afterEach, beforeEach, expect, test, vi } from 'vite-plus/test';
import { Zedflow } from '../src/index.ts';

const cleanups: (() => void)[] = [];
beforeEach(() => {
  vi.spyOn(globalThis, 'fetch').mockRejectedValue(
    new Error('Network forbidden in native conversation tests'),
  );
});
afterEach(() => {
  cleanups.splice(0).forEach((cleanup) => cleanup());
  expect(globalThis.fetch).not.toHaveBeenCalled();
  vi.restoreAllMocks();
});
const message = (
  model: Model<Api>,
  content: AssistantMessage['content'],
  stopReason: 'stop' | 'toolUse' = 'stop',
): AssistantMessage => ({
  role: 'assistant',
  content,
  stopReason,
  api: model.api,
  provider: model.provider,
  model: model.id,
  timestamp: Date.now(),
  usage: {
    input: 0,
    output: 0,
    cacheRead: 0,
    cacheWrite: 0,
    totalTokens: 0,
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
  },
});
const response =
  (
    content: AssistantMessage['content'],
    reason: 'stop' | 'toolUse' = 'stop',
  ): Provider['streamSimple'] =>
  (model) => {
    const stream = createAssistantMessageEventStream();
    const result = message(model, content, reason);
    stream.push({ type: 'start', partial: result });
    stream.push({ type: 'done', reason, message: result });
    return stream;
  };
const answer = () => response([{ type: 'text', text: 'fixture answer' }]);
const call = (
  name = 'read',
  args: JsonObject = { path: 'fixture.txt' },
  id = 'call-1',
): ToolCall => ({ type: 'toolCall', name, arguments: args, id });
const calls = (...items: ToolCall[]) => response(items, 'toolUse');

const fixture = async (
  steps: readonly Provider['streamSimple'][],
  maxModelCalls = 4,
  tools: readonly string[] = ['read', 'write', 'bash'],
) => {
  const root = resolve('.artifacts/legacy-migration/native-conversations');
  await mkdir(root, { recursive: true });
  const base = await mkdtemp(join(root, 'case-'));
  const cwd = join(base, 'project');
  await mkdir(cwd);
  await writeFile(join(cwd, 'fixture.txt'), 'fixture');
  const conversation = new ConversationProvider(steps);
  const { requests, provider } = conversation;
  const app = new Zedflow({
    cwd,
    globalDirectory: join(base, 'global'),
    recordsDirectory: join(base, 'records'),
    provider,
    authContext: { env: async () => undefined, fileExists: async () => false },
    overrides: { maxModelCalls, tools: [...tools] },
  });
  const start = async () => {
    const id = await app.launch('agent');
    cleanups.push(() => app.runs.stop(id));
    app.runs.send(id, { type: 'PROMPT', text: 'fixture prompt' });
    return id;
  };
  const context = (id: string) =>
    (app.runs.view(id) as unknown as { context: AgentContext }).context;
  const ready = (id: string, modelCalls = 2) =>
    vi.waitFor(() =>
      expect(app.runs.view(id)).toMatchObject({
        value: 'ready',
        context: { modelCalls, failure: null },
      }),
    );
  return { app, cwd, requests, start, context, ready };
};

test('native CLI conversation performs model → real built-in → model and retains standard transcript order', async () => {
  const f = await fixture([calls(call()), answer(), answer()]);
  const id = await f.start();
  await f.ready(id);
  expect(f.context(id).messages.map((item) => item.role)).toEqual([
    'user',
    'assistant',
    'toolResult',
    'assistant',
  ]);
  expect(withoutInitialSystemMessage(f.requests[1]?.messages ?? [])[2]).toMatchObject({
    toolCallId: 'call-1',
    toolName: 'read',
    isError: false,
    content: [{ type: 'text', text: 'fixture' }],
  });
  f.app.runs.send(id, { type: 'PROMPT', text: 'second turn' });
  await f.ready(id, 1);
  expect(f.context(id).messages).toHaveLength(6);
});

test.each([call('unknown'), call('guarded', {})])(
  'native invalid tool calls yield an error result without executing a handler: %j',
  async (invalid) => {
    const f = await fixture([calls(invalid), answer()], 4, ['guarded']);
    const directory = join(f.cwd, '.zedflow', 'tools', 'guarded');
    await mkdir(directory, { recursive: true });
    await writeFile(
      join(directory, 'tool.json'),
      JSON.stringify({
        name: 'guarded',
        inputSchema: {
          type: 'object',
          properties: { text: { type: 'string' } },
          required: ['text'],
        },
        execution: {
          command: process.execPath,
          args: ['-e', "require('node:fs').writeFileSync('invalid-handler-ran', 'unexpected');"],
        },
      }),
    );
    const execute = vi.spyOn(CommandTool.prototype, 'execute');
    const id = await f.start();
    await f.ready(id);
    expect(f.context(id).messages[2]).toMatchObject({
      role: 'toolResult',
      toolCallId: invalid.id,
      toolName: invalid.name,
      isError: true,
    });
    expect(execute).not.toHaveBeenCalled();
    expect(await readFile(join(f.cwd, 'fixture.txt'), 'utf8')).toBe('fixture');
    await expect(readFile(join(f.cwd, 'invalid-handler-ran'))).rejects.toMatchObject({
      code: 'ENOENT',
    });
  },
);

test('native multiple calls execute sequentially and preserve their identifiers', async () => {
  const f = await fixture([
    calls(
      call('write', { path: 'sequential.txt', content: 'first' }),
      call('read', { path: 'sequential.txt' }, 'call-2'),
    ),
    answer(),
  ]);
  const id = await f.start();
  await f.ready(id);
  expect(f.context(id).messages.slice(2, 4)).toMatchObject([
    { toolCallId: 'call-1' },
    { toolCallId: 'call-2', content: [{ type: 'text', text: 'first' }] },
  ]);
  expect(await readFile(join(f.cwd, 'sequential.txt'), 'utf8')).toBe('first');
});

test('native model-call budget terminates a tool loop without an unbounded continuation', async () => {
  const f = await fixture(
    Array.from({ length: 3 }, (_, index) =>
      calls(call('read', { path: 'fixture.txt' }, `budget-${index}`)),
    ),
    2,
  );
  const id = await f.start();
  await vi.waitFor(() =>
    expect(f.app.runs.view(id)).toMatchObject({
      value: 'failed',
      context: { modelCalls: 2, toolCalls: 1, failure: 'Model-call budget exhausted' },
    }),
  );
  expect(f.requests).toHaveLength(2);
});

test('native cancellation aborts model work and ignores its late result after reset', async () => {
  let signal: AbortSignal | undefined;
  let finish: (() => void) | undefined;
  const f = await fixture([
    (model, _context, options) => {
      signal = options?.signal;
      const stream = createAssistantMessageEventStream();
      const result = message(
        model,
        [call('write', { path: 'late-effect', content: 'must not run' })],
        'toolUse',
      );
      stream.push({ type: 'start', partial: result });
      finish = () => stream.push({ type: 'done', reason: 'toolUse', message: result });
      return stream;
    },
  ]);
  const id = await f.start();
  await vi.waitFor(() => expect(signal).toBeDefined());
  f.app.runs.send(id, { type: 'CANCEL' });
  expect(signal?.aborted).toBe(true);
  f.app.runs.send(id, { type: 'RESET' });
  finish?.();
  await delay(20);
  expect(f.app.runs.view(id)).toMatchObject({
    value: 'ready',
    context: { messages: [], pending: [], modelCalls: 0 },
  });
  await expect(readFile(join(f.cwd, 'late-effect'))).rejects.toMatchObject({ code: 'ENOENT' });
});

test('native cancellation terminates an active real tool process and does not request another model turn', async () => {
  const script =
    "require('node:fs').writeFileSync('tool-pid', String(process.pid)); setTimeout(() => {}, 10000);";
  const f = await fixture([
    calls(
      call('bash', { command: `${JSON.stringify(process.execPath)} -e ${JSON.stringify(script)}` }),
    ),
  ]);
  const id = await f.start();
  let pid = 0;
  await vi.waitFor(async () => {
    pid = Number(await readFile(join(f.cwd, 'tool-pid'), 'utf8'));
    expect(Number.isSafeInteger(pid) && pid > 0).toBe(true);
  });
  expect(f.app.runs.view(id)).toMatchObject({ value: 'tool' });
  f.app.runs.send(id, { type: 'CANCEL' });
  await vi.waitFor(() => expect(() => process.kill(pid, 0)).toThrow());
  expect(f.app.runs.view(id)).toMatchObject({ value: 'cancelled' });
  expect(f.requests).toHaveLength(1);
});

test('native model failures are explicit and reset starts a clean interaction', async () => {
  const f = await fixture([
    () => {
      throw new Error('fixture failure');
    },
  ]);
  const id = await f.start();
  await vi.waitFor(() => expect(f.app.runs.view(id)).toMatchObject({ value: 'failed' }));
  expect(f.context(id).failure).toContain('fixture failure');
  f.app.runs.send(id, { type: 'RESET' });
  expect(f.context(id)).toMatchObject({
    messages: [],
    pending: [],
    modelCalls: 0,
    toolCalls: 0,
    failure: null,
  });
});

test('native agent isolates provider requests and declares tools; Pi Models skips pre-aborted transport', async () => {
  const f = await fixture([
    (model, context) => {
      const user = context.messages.find((item) => item.role === 'user');
      if (!user) {
        throw new Error('Missing native user message');
      }
      user.content = 'provider mutation';
      return answer()(model, context);
    },
  ]);
  const id = await f.start();
  await f.ready(id, 1);
  expect(f.requests).toHaveLength(1);
  expect(f.context(id).messages[0]).toMatchObject({ role: 'user', content: 'fixture prompt' });
  expect(getCurrentTools(f.requests[0]?.messages ?? []).map((tool) => tool.name)).toEqual([
    'read',
    'write',
    'bash',
    'codemode',
  ]);
  const stream = f.app.runtime.models.streamSimple(
    f.app.runtime.select('openai', 'gpt-6.1-sol'),
    { messages: [] },
    { signal: AbortSignal.abort() },
  );
  expect((await stream.result()).stopReason).toBe('aborted');
  expect(f.requests).toHaveLength(1);
});

test('native Models preserves the caller cancellation signal through its configured provider', async () => {
  const controller = new AbortController();
  cleanups.push(() => controller.abort());
  let received: AbortSignal | undefined;
  const f = await fixture([
    (model, _context, options) => {
      received = options?.signal;
      const stream = createAssistantMessageEventStream();
      const partial = message(model, []);
      stream.push({ type: 'start', partial });
      received?.addEventListener(
        'abort',
        () =>
          stream.push({
            type: 'error',
            reason: 'aborted',
            error: { ...partial, stopReason: 'aborted' },
          }),
        { once: true },
      );
      return stream;
    },
  ]);
  const stream = f.app.runtime.models.streamSimple(
    f.app.runtime.select('openai', 'gpt-6.1-sol'),
    { messages: [] },
    { signal: controller.signal },
  );
  await vi.waitFor(() => expect(received).toBe(controller.signal));
  controller.abort();
  expect((await stream.result()).stopReason).toBe('aborted');
  expect(f.requests).toHaveLength(1);
});

test('root facade ships the native agent through ordinary manifest discovery and coordinator records', async () => {
  const f = await fixture([answer()]);
  expect((await f.app.resources.list()).get('agent')).toMatchObject({
    id: 'agent',
    scope: 'builtin',
    manifest: { entry: 'machine.ts' },
  });
  const id = await f.start();
  await f.ready(id, 1);
  expect(f.app.runs.records(id)[0]?.data).toMatchObject({
    path: 'machine:agent',
    machine: 'zf.agent',
  });
  expect(f.app.runs.records(id)[0]?.data).not.toHaveProperty('builtin');
});
