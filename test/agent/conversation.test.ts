import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { getCurrentTools, withoutInitialSystemMessage } from '@earendil-works/pi-ai';
import { expect, test } from 'vite-plus/test';
import { createMachine } from '../../packages/machines/agent/machine.ts';
import { BuiltinConversation } from '../support/agent/builtin-conversation.ts';
import { fixture, startMachine, context, ready } from '../support/agent/execution.ts';
import { png, results, text } from '../support/agent/messages.ts';
import { NativeOpenAI } from '../support/agent/native-openai.ts';
import { answer } from '../support/agent/responses.ts';

const expectBuiltinResults = (native: NativeOpenAI) => {
  const toolResults = results(native.request(1));
  expect(toolResults.map((result) => result.toolCallId)).toEqual([
    'write-1',
    'read-1',
    'image-1',
    'bash-1',
  ]);
  expect(text(toolResults[1])).toBe('written by agent');
  expect(toolResults[2]?.content).toContainEqual({
    type: 'image',
    mimeType: 'image/png',
    data: png,
  });
  expect(toolResults.every((result) => typeof result.durationMs === 'number')).toBe(true);
};

test.each([
  { type: 'CANCEL' },
  { type: 'PROMPT', text: '' },
  { type: 'PROMPT', text: ' \n\t' },
] as const)('agent treats idle input %j as a no-op without stranding history', async (event) => {
  const cwd = await fixture();
  const native = await NativeOpenAI.create([
    answer('completed answer'),
    answer('continued answer'),
  ]);
  const { execution } = startMachine(createMachine(native.bindings(cwd)));
  const initial = execution.view();
  execution.send(event);
  expect(execution.view()).toEqual(initial);
  expect(native.requestCount()).toBe(0);
  execution.send({ type: 'PROMPT', text: 'complete a turn' });
  await ready(execution, 1);
  const completed = execution.view();
  const history = context(execution).messages;
  execution.send(event);
  expect(execution.view()).toEqual(completed);
  expect(native.requestCount()).toBe(1);
  execution.send({ type: 'PROMPT', text: 'continue without reset' });
  await ready(execution, 1);
  expect(context(execution).messages).toHaveLength(4);
  expect(context(execution).messages.slice(0, history.length)).toEqual(history);
  expect(withoutInitialSystemMessage(native.request(1).messages).slice(0, history.length)).toEqual(
    history,
  );
  execution.send({ type: 'RESET' });
  expect(execution.view()).toEqual(initial);
});

test('agent performs native model → real built-ins → model, preserves images and streams, and reuses its conversation', async () => {
  const cwd = await fixture();
  const { native, tools, execution, frames, modelEvents, updates } =
    await BuiltinConversation.create(cwd);
  execution.send({ type: 'PROMPT', text: 'Use the supplied tools' });
  await ready(execution, 2);
  expect(await readFile(join(cwd, 'file.txt'), 'utf8')).toBe('written by agent');
  expect(context(execution).messages.map((item) => item.role)).toEqual([
    'user',
    'assistant',
    'toolResult',
    'toolResult',
    'toolResult',
    'toolResult',
    'assistant',
  ]);
  expect(context(execution).toolCalls).toBe(4);
  expect(withoutInitialSystemMessage(native.request(1).messages)).toEqual(
    context(execution).messages.slice(0, -1),
  );
  expectBuiltinResults(native);
  expect(getCurrentTools(native.request(0).messages).map((tool) => tool.name)).toEqual(
    tools.map((tool) => tool.declaration.name),
  );
  expect(
    modelEvents.some((event) => event.type === 'text_delta' && event.delta === 'fixture answer'),
  ).toBe(true);
  expect(modelEvents.some((event) => event.type === 'toolcall_end')).toBe(true);
  expect(updates.some((update) => update.toolName === 'bash')).toBe(true);
  execution.send({ type: 'PROMPT', text: 'Continue this conversation' });
  await ready(execution, 1);
  expect(context(execution).messages).toHaveLength(9);
  expect(context(execution).toolCalls).toBe(0);
  expect(withoutInitialSystemMessage(native.request(2).messages)).toHaveLength(8);
  const persisted = JSON.stringify({ checkpoint: execution.checkpoint(), frames });
  for (const forbidden of ['offline-agent-fixture', 'streamSimple', 'credentials', 'baseUrl']) {
    expect(persisted).not.toContain(forbidden);
  }
  expect(frames.some((frame) => frame.type === 'serialization.error')).toBe(false);
});
