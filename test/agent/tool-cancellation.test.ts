import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { withoutInitialSystemMessage } from '@earendil-works/pi-ai';
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import { expect, test, vi } from 'vite-plus/test';
import { createMachine } from '../../packages/machines/agent/machine.ts';
import type { AgentContext } from '../../packages/machines/agent/machine.ts';
import { AbortIgnoringTool } from '../support/agent/abort-ignoring-tool.ts';
import { fixture, startMachine, context, ready } from '../support/agent/execution.ts';
import { InterruptedBatch } from '../support/agent/interrupted-batch.ts';
import { call, results, text } from '../support/agent/messages.ts';
import { NativeOpenAI, providerInput } from '../support/agent/native-openai.ts';
import { answer, calls } from '../support/agent/responses.ts';

const expectRepairedBatch = (native: NativeOpenAI, history: AgentContext['messages']) => {
  const request = native.request(2);
  expect(withoutInitialSystemMessage(request.messages).slice(0, history.length)).toEqual(history);
  expect(results(request).map((item) => item.toolCallId)).toEqual(['completed-call']);
  const input = providerInput(native, 2);
  expect(input.filter((item) => item.type === 'function_call').map((item) => item.call_id)).toEqual(
    ['completed-call', 'active-call', 'skipped-call'],
  );
  expect(
    input.filter((item) => item.type === 'function_call_output').map((item) => item.call_id),
  ).toEqual(['completed-call', 'active-call', 'skipped-call']);
  expect(input).toContainEqual({
    type: 'function_call_output',
    call_id: 'active-call',
    output: 'No result provided',
  });
  expect(input).toContainEqual({
    type: 'function_call_output',
    call_id: 'skipped-call',
    output: 'No result provided',
  });
  expect(input.at(-1)).toMatchObject({
    role: 'user',
    content: [{ type: 'input_text', text: 'continue the conversation' }],
  });
};

test.each([
  ['CANCEL', false],
  ['CANCEL', true],
  ['RESET', false],
  ['RESET', true],
] as const)(
  'agent ignores late abort-ignoring tool updates/results after %s (reject=%s)',
  async (event, reject) => {
    const cwd = await fixture();
    const tool = new AbortIgnoringTool(cwd, reject);
    const native = await NativeOpenAI.create([
      calls(call('custom', { value: 'wait' })),
      answer('fresh turn'),
    ]);
    const updates: ToolResultMessage[] = [];
    const { execution } = startMachine(
      createMachine(
        native.bindings(cwd, { tools: [tool], onToolUpdate: (update) => updates.push(update) }),
      ),
    );
    execution.send({ type: 'PROMPT', text: 'old turn' });
    await vi.waitFor(() => expect(tool.entered).toBe(true));
    execution.send({ type: event });
    expect(tool.signal?.aborted).toBe(true);
    expect(context(execution).pending).toEqual([]);
    if (event === 'CANCEL') {
      expect(context(execution).failure).toContain('effects may be incomplete');
    }
    const history = context(execution).messages;
    execution.send({ type: 'PROMPT', text: 'fresh prompt' });
    await ready(execution, 1);
    expect(context(execution)).toMatchObject({ pending: [], toolCalls: 0, failure: null });
    expect(context(execution).messages.slice(0, history.length)).toEqual(history);
    const fresh = execution.view();
    tool.release();
    await vi.waitFor(() => expect(tool.completed).toBe(true));
    await delay(20);
    expect(await readFile(join(cwd, 'late-effect'), 'utf8')).toBe('effects cannot be rolled back');
    expect(updates.map((update) => text(update))).toEqual(['before abort']);
    expect(execution.view()).toEqual(fresh);
    expect(native.requestCount()).toBe(2);
  },
);

test('agent continues an unfinished cancelled tool batch with completed history and native provider repair, without replaying tools', async () => {
  const cwd = await fixture();
  const { wait, native, execution } = await InterruptedBatch.create(cwd);
  execution.send({ type: 'PROMPT', text: 'seed the conversation' });
  await ready(execution, 1);
  execution.send({ type: 'PROMPT', text: 'start a tool batch' });
  await vi.waitFor(() => expect(wait.entered).toBe(true));
  expect(context(execution).toolCalls).toBe(2);
  expect(context(execution).pending.map((item) => item.id)).toEqual([
    'active-call',
    'skipped-call',
  ]);
  const history = context(execution).messages;
  expect(history).toHaveLength(5);
  execution.send({ type: 'CANCEL' });
  const cancelled = execution.view();
  expect(cancelled).toMatchObject({ value: 'cancelled', context: { pending: [] } });
  execution.send({ type: 'CANCEL' });
  execution.send({ type: 'PROMPT', text: ' ' });
  expect(execution.view()).toEqual(cancelled);
  execution.send({ type: 'PROMPT', text: 'continue the conversation' });
  await ready(execution, 1);
  await delay(20);
  expect(context(execution)).toMatchObject({ pending: [], toolCalls: 0, failure: null });
  expect(context(execution).messages).toHaveLength(7);
  expect(context(execution).messages.slice(0, history.length)).toEqual(history);
  expectRepairedBatch(native, history);
  expect(await readFile(join(cwd, 'completed'), 'utf8')).toBe('kept');
  await expect(readFile(join(cwd, 'skipped'))).rejects.toMatchObject({ code: 'ENOENT' });
  expect(native.requestCount()).toBe(3);
});
