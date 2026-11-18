import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { withoutInitialSystemMessage } from '@earendil-works/pi-ai';
import { expect, test, vi } from 'vite-plus/test';
import { createMachine } from '../../packages/machines/agent/machine.ts';
import { createWriteTool } from '../../packages/tools/src/index.ts';
import { fixture, startMachine, context, ready } from '../support/agent/execution.ts';
import { call, results } from '../support/agent/messages.ts';
import { NativeOpenAI, providerInput } from '../support/agent/native-openai.ts';
import { answer, calls } from '../support/agent/responses.ts';

test('agent enforces the manifest default model-call budget and reserves a final model request before tool effects', async () => {
  const cwd = await fixture();
  const native = await NativeOpenAI.create([
    ...Array.from({ length: 4 }, (_, index) =>
      calls(call('write', { path: `effect-${index}`, content: String(index) }, `write-${index}`)),
    ),
    answer('continued after model budget'),
  ]);
  const { execution } = startMachine(
    createMachine(native.bindings(cwd, { tools: [createWriteTool(cwd)] })),
  );
  execution.send({ type: 'PROMPT', text: 'keep requesting tools' });
  await vi.waitFor(() =>
    expect(execution.view()).toMatchObject({
      value: 'failed',
      context: { modelCalls: 4, toolCalls: 3, pending: [], failure: 'Model-call budget exhausted' },
    }),
  );
  expect(native.requestCount()).toBe(4);
  await expect(readFile(join(cwd, 'effect-3'))).rejects.toMatchObject({ code: 'ENOENT' });
  expect(await readFile(join(cwd, 'effect-2'), 'utf8')).toBe('2');
  const history = context(execution).messages;
  execution.send({ type: 'PROMPT', text: 'continue after the model-call budget' });
  await ready(execution, 1);
  expect(context(execution)).toMatchObject({ pending: [], toolCalls: 0, failure: null });
  expect(context(execution).messages.slice(0, history.length)).toEqual(history);
  expect(withoutInitialSystemMessage(native.request(4).messages).slice(0, history.length)).toEqual(
    history,
  );
  expect(providerInput(native, 4)).toContainEqual({
    type: 'function_call_output',
    call_id: 'write-3',
    output: 'No result provided',
  });
  expect(results(native.request(4))).toHaveLength(3);
  await expect(readFile(join(cwd, 'effect-3'))).rejects.toMatchObject({ code: 'ENOENT' });
});

test.each(['batch', 'cumulative'] as const)(
  'agent enforces its %s tool-call budget before over-budget effects',
  async (budget) => {
    const cwd = await fixture();
    const one = call('write', { path: 'one', content: 'first' }, 'one');
    const two = call('write', { path: 'two', content: 'second' }, 'two');
    const native = await NativeOpenAI.create([
      ...(budget === 'batch' ? [calls(one, two)] : [calls(one), calls(two)]),
      answer('continued after tool budget'),
    ]);
    const { execution } = startMachine(
      createMachine(
        native.bindings(cwd, { tools: [createWriteTool(cwd)], config: { maxToolCalls: 1 } }),
      ),
    );
    execution.send({ type: 'PROMPT', text: 'bounded tools' });
    await vi.waitFor(() =>
      expect(execution.view()).toMatchObject({
        value: 'failed',
        context: {
          modelCalls: budget === 'batch' ? 1 : 2,
          toolCalls: budget === 'batch' ? 0 : 1,
          pending: [],
          failure: 'Tool-call budget exhausted',
        },
      }),
    );
    await expect(readFile(join(cwd, 'two'))).rejects.toMatchObject({ code: 'ENOENT' });
    if (budget === 'cumulative') {
      expect(await readFile(join(cwd, 'one'), 'utf8')).toBe('first');
    } else {
      await expect(readFile(join(cwd, 'one'))).rejects.toMatchObject({ code: 'ENOENT' });
    }
    const history = context(execution).messages;
    execution.send({ type: 'PROMPT', text: 'continue after the tool-call budget' });
    await ready(execution, 1);
    expect(context(execution)).toMatchObject({ pending: [], toolCalls: 0, failure: null });
    expect(context(execution).messages.slice(0, history.length)).toEqual(history);
    const requestIndex = budget === 'batch' ? 1 : 2;
    expect(
      withoutInitialSystemMessage(native.request(requestIndex).messages).slice(0, history.length),
    ).toEqual(history);
    expect(providerInput(native, requestIndex)).toContainEqual({
      type: 'function_call_output',
      call_id: 'two',
      output: 'No result provided',
    });
    expect(results(native.request(requestIndex))).toHaveLength(budget === 'batch' ? 0 : 1);
    await expect(readFile(join(cwd, 'two'))).rejects.toMatchObject({ code: 'ENOENT' });
  },
);
