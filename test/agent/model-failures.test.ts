import {
  createAssistantMessageEventStream,
  withoutInitialSystemMessage,
} from '@earendil-works/pi-ai';
import { expect, test, vi } from 'vite-plus/test';
import { createMachine } from '../../packages/machines/agent/machine.ts';
import { fixture, startMachine, context, ready } from '../support/agent/execution.ts';
import { message } from '../support/agent/messages.ts';
import { NativeOpenAI, providerInput } from '../support/agent/native-openai.ts';
import { response, answer } from '../support/agent/responses.ts';

test.each(['error', 'aborted', 'length', 'deferred'] as const)(
  'agent handles native model stop reason %s without reporting success',
  async (reason) => {
    const cwd = await fixture();
    const native = await NativeOpenAI.create([
      answer('completed turn'),
      response([{ type: 'text', text: 'partial response' }], reason),
      answer('after continuation'),
    ]);
    const { execution } = startMachine(createMachine(native.bindings(cwd)));
    execution.send({ type: 'PROMPT', text: 'complete a turn' });
    await ready(execution, 1);
    execution.send({ type: 'PROMPT', text: 'fail this turn' });
    await vi.waitFor(() =>
      expect(execution.view()).toMatchObject({
        value: reason === 'aborted' ? 'cancelled' : 'failed',
        context: { modelCalls: 1, toolCalls: 0 },
      }),
    );
    expect(context(execution).failure).toContain(reason);
    expect(context(execution).messages).toHaveLength(4);
    const failed = execution.view();
    const history = context(execution).messages;
    execution.send({ type: 'CANCEL' });
    execution.send({ type: 'PROMPT', text: ' \n\t' });
    expect(execution.view()).toEqual(failed);
    execution.send({ type: 'PROMPT', text: 'try again without resetting' });
    await ready(execution, 1);
    expect(context(execution)).toMatchObject({ pending: [], toolCalls: 0, failure: null });
    expect(context(execution).messages).toHaveLength(6);
    expect(context(execution).messages.slice(0, history.length)).toEqual(history);
    expect(withoutInitialSystemMessage(native.request(2).messages)).toEqual([
      ...history,
      expect.objectContaining({ role: 'user', content: 'try again without resetting' }),
    ]);
    const input = JSON.stringify(providerInput(native, 2));
    expect(input).toContain('completed turn');
    if (reason === 'error' || reason === 'aborted') {
      expect(input).not.toContain('partial response');
    } else {
      expect(input).toContain('partial response');
    }
  },
);

test('agent handles native provider exceptions and missing in-memory auth without network or credentials in state', async () => {
  const cwd = await fixture();
  for (const authenticated of [true, false]) {
    const native = await NativeOpenAI.create(
      [
        () => {
          throw new Error('fixture transport failed');
        },
      ],
      authenticated,
    );
    const { execution } = startMachine(createMachine(native.bindings(cwd)));
    execution.send({ type: 'PROMPT', text: 'request a model' });
    await vi.waitFor(() =>
      expect(execution.view()).toMatchObject({ value: 'failed', context: { modelCalls: 1 } }),
    );
    expect(context(execution).failure).toContain(
      authenticated ? 'fixture transport failed' : 'not configured',
    );
    expect(native.requestCount()).toBe(authenticated ? 1 : 0);
    expect(JSON.stringify(execution.checkpoint())).not.toContain('offline-agent-fixture');
  }
});

test('agent rejects non-JSON model data before it reaches persisted history', async () => {
  const cwd = await fixture();
  const native = await NativeOpenAI.create([
    (model) => {
      const stream = createAssistantMessageEventStream();
      const result = message(model, [{ type: 'text', text: 'invalid usage' }]);
      result.usage.output = Number.POSITIVE_INFINITY;
      stream.push({ type: 'start', partial: result });
      stream.push({ type: 'done', reason: 'stop', message: result });
      return stream;
    },
  ]);
  const { execution, frames } = startMachine(createMachine(native.bindings(cwd)));
  execution.send({ type: 'PROMPT', text: 'reject non-JSON output' });
  await vi.waitFor(() => expect(execution.view()).toMatchObject({ value: 'failed' }));
  expect(context(execution).failure).toContain('Non-finite JSON payload');
  expect(context(execution).messages).toHaveLength(1);
  expect(frames.some((frame) => frame.type === 'serialization.error')).toBe(false);
  expect(() => execution.checkpoint()).not.toThrow();
});
