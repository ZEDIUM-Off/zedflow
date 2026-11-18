import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool } from '@zf/core';
import { expect, test, vi } from 'vite-plus/test';
import { createMachine } from '../../packages/machines/agent/machine.ts';
import { createEditTool } from '../../packages/tools/src/index.ts';
import { fixture, startMachine, context, ready } from '../support/agent/execution.ts';
import { call, results, text, resultFor, customDeclaration } from '../support/agent/messages.ts';
import { NativeOpenAI } from '../support/agent/native-openai.ts';
import { PreparedTool } from '../support/agent/prepared-tool.ts';
import { answer, calls } from '../support/agent/responses.ts';

test.each([
  { path: 'file', oldText: 'before', newText: 'after' },
  { path: 'file', edits: '[{"oldText":"before","newText":"after"}]' },
  { path: 'file', edits: { oldText: 'before', newText: 'after' } },
])(
  'agent prepares native edit repair before schema validation without changing history: %j',
  async (args) => {
    const cwd = await fixture();
    await writeFile(join(cwd, 'file'), 'before');
    const original = structuredClone(args);
    const native = await NativeOpenAI.create([calls(call('edit', args)), answer()]);
    const { execution } = startMachine(
      createMachine(native.bindings(cwd, { tools: [createEditTool(cwd)] })),
    );
    execution.send({ type: 'PROMPT', text: 'Repair this edit call' });
    await ready(execution, 2);
    expect(await readFile(join(cwd, 'file'), 'utf8')).toBe('after');
    const historicalCall = context(execution).messages.flatMap((item) =>
      item.role === 'assistant' ? item.content.filter((block) => block.type === 'toolCall') : [],
    )[0];
    expect(historicalCall?.arguments).toEqual(original);
    expect(args).toEqual(original);
    expect(results(native.request(1))[0]).toMatchObject({
      isError: false,
      details: { firstChangedLine: 1 },
    });
  },
);

test('agent prepares arbitrary injected tools once, validates/coerces afterwards, preserves rich native results and suppresses finalized updates', async () => {
  const cwd = await fixture();
  const originalArgs = { value: 'raw', count: '2' };
  const tool = new PreparedTool();
  const { rich } = tool;
  const native = await NativeOpenAI.create([calls(call('custom', originalArgs)), answer()]);
  const updates: ToolResultMessage[] = [];
  const { execution } = startMachine(
    createMachine(
      native.bindings(cwd, { tools: [tool], onToolUpdate: (update) => updates.push(update) }),
    ),
  );
  execution.send({ type: 'PROMPT', text: 'Use a custom executable' });
  await ready(execution, 2);
  expect(tool.preparations).toBe(1);
  expect(results(native.request(1))).toEqual([rich]);
  expect(context(execution).messages.filter((item) => item.role === 'toolResult')).toEqual([rich]);
  expect(originalArgs).toEqual({ value: 'raw', count: '2' });
  expect(
    native.request(1).messages.flatMap((item) => (item.role === 'assistant' ? item.content : [])),
  ).toContainEqual(call('custom', originalArgs));
  expect(updates).toEqual([rich]);
  tool.emitFinalizedUpdate();
  expect(updates).toEqual([rich]);
});

test.each([{}, { value: { invalid: true } }])(
  'agent validates arbitrary unprepared tools before execution: %j',
  async (args) => {
    const cwd = await fixture();
    const execute = vi.fn(async ({ call: input }: Parameters<ExecutableTool['execute']>[0]) =>
      resultFor(input),
    );
    const native = await NativeOpenAI.create([calls(call('custom', args)), answer()]);
    const { execution } = startMachine(
      createMachine(native.bindings(cwd, { tools: [{ declaration: customDeclaration, execute }] })),
    );
    execution.send({ type: 'PROMPT', text: 'Reject invalid arguments' });
    await ready(execution, 2);
    expect(execute).not.toHaveBeenCalled();
    expect(results(native.request(1))[0]?.isError).toBe(true);
    expect(text(results(native.request(1))[0])).toContain('Validation failed');
  },
);

test('agent validates arbitrary normalized arguments instead of trusting the preparation hook', async () => {
  const cwd = await fixture();
  const execute = vi.fn(async ({ call: input }: Parameters<ExecutableTool['execute']>[0]) =>
    resultFor(input),
  );
  const tool: ExecutableTool = {
    declaration: customDeclaration,
    prepareArguments: () => ({ value: { invalid: true } }),
    execute,
  };
  const native = await NativeOpenAI.create([
    calls(call('custom', { value: 'initially valid' })),
    answer(),
  ]);
  const { execution } = startMachine(createMachine(native.bindings(cwd, { tools: [tool] })));
  execution.send({ type: 'PROMPT', text: 'validate after preparation' });
  await ready(execution, 2);
  expect(execute).not.toHaveBeenCalled();
  expect(results(native.request(1))[0]?.isError).toBe(true);
  expect(text(results(native.request(1))[0])).toContain('Validation failed');
});
