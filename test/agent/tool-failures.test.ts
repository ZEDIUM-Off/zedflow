import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { getCurrentTools } from '@earendil-works/pi-ai';
import type { ExecutableTool } from '@zf/core';
import { expect, test } from 'vite-plus/test';
import { createMachine } from '../../packages/machines/agent/machine.ts';
import { createReadTool } from '../../packages/tools/src/index.ts';
import { fixture, startMachine, ready } from '../support/agent/execution.ts';
import { call, results, text, resultFor, customDeclaration } from '../support/agent/messages.ts';
import { NativeOpenAI } from '../support/agent/native-openai.ts';
import { answer, calls } from '../support/agent/responses.ts';

test('agent refuses tools not supplied by the CLI, even when they are built-in names', async () => {
  const cwd = await fixture();
  const native = await NativeOpenAI.create([
    calls(call('write', { path: 'not-created', content: 'forbidden' })),
    answer(),
  ]);
  const { execution } = startMachine(
    createMachine(native.bindings(cwd, { tools: [createReadTool(cwd)] })),
  );
  execution.send({ type: 'PROMPT', text: 'Respect the selected tools' });
  await ready(execution, 2);
  expect(getCurrentTools(native.request(0).messages).map((tool) => tool.name)).toEqual(['read']);
  expect(results(native.request(1))[0]?.isError).toBe(true);
  expect(text(results(native.request(1))[0])).toContain('Tool write not found');
  await expect(readFile(join(cwd, 'not-created'))).rejects.toMatchObject({ code: 'ENOENT' });
});

test.each(['prepare', 'execute', 'correlation'] as const)(
  'agent returns a native error result for %s failures so the model can recover',
  async (failure) => {
    const cwd = await fixture();
    const tool: ExecutableTool = {
      declaration: customDeclaration,
      prepareArguments: (args) => {
        if (failure === 'prepare') {
          throw new Error('fixture preparation failed');
        }
        return args;
      },
      execute: async ({ call: input }) => {
        if (failure === 'execute') {
          throw new Error('fixture execution failed');
        }
        return { ...resultFor(input), toolCallId: 'wrong-id' };
      },
    };
    const native = await NativeOpenAI.create([
      calls(call('custom', { value: 'fixture' })),
      answer('recovered'),
    ]);
    const { execution } = startMachine(createMachine(native.bindings(cwd, { tools: [tool] })));
    execution.send({ type: 'PROMPT', text: 'Handle tool errors' });
    await ready(execution, 2);
    expect(results(native.request(1))[0]).toMatchObject({
      toolCallId: 'call-1',
      toolName: 'custom',
      isError: true,
    });
    expect(text(results(native.request(1))[0])).toContain(
      failure === 'correlation' ? 'does not match' : 'fixture',
    );
  },
);
