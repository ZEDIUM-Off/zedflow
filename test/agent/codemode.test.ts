import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { getCurrentTools } from '@earendil-works/pi-ai';
import { expect, test } from 'vite-plus/test';
import { createMachine } from '../../packages/machines/agent/machine.ts';
import { createCodemodeTool, createWriteTool } from '../../packages/tools/src/index.ts';
import { CodemodeProfile } from '../support/agent/codemode-profile.ts';
import { fixture, startMachine, context, ready } from '../support/agent/execution.ts';
import { png, call, results, text } from '../support/agent/messages.ts';
import { NativeOpenAI } from '../support/agent/native-openai.ts';
import { answer, calls } from '../support/agent/responses.ts';

test.each(['on', 'only'] as const)(
  'agent uses the actual Code Mode VM with the CLI-prepared %s profile and intact script results',
  async (mode) => {
    const cwd = await fixture();
    const { native, tools, execution, updates } = await CodemodeProfile.create(cwd, mode);
    execution.send({ type: 'PROMPT', text: 'Use Code Mode' });
    await ready(execution, 2);
    expect(getCurrentTools(native.request(0).messages).map((tool) => tool.name)).toEqual(
      tools.map((tool) => tool.declaration.name),
    );
    expect(await readFile(join(cwd, 'vm.txt'), 'utf8')).toBe('after');
    const result = results(native.request(1))[0];
    expect(result).toMatchObject({
      toolCallId: 'script-1',
      toolName: 'codemode',
      isError: false,
      nestedCalls: {
        complete: true,
        calls: [
          { name: 'write', status: 'ok' },
          { name: 'edit', status: 'ok' },
          { name: 'read', status: 'ok' },
          { name: 'bash', status: 'error' },
          { name: 'read', status: 'ok' },
        ],
      },
    });
    expect(text(result)).toContain('"exitCode":3');
    expect(text(result)).toContain('"value":"after"');
    expect(result?.content).toContainEqual({ type: 'image', mimeType: 'image/png', data: png });
    expect(
      updates.some((update) => update.nestedCalls?.calls.some((nested) => nested.name === 'bash')),
    ).toBe(true);
    expect(context(execution).messages.filter((item) => item.role === 'toolResult')).toEqual([
      result,
    ]);
  },
);

test.each(['exception', 'nested-budget'] as const)(
  'agent preserves actual Code Mode %s errors and completed effects for model recovery',
  async (failure) => {
    const cwd = await fixture();
    const codemode = createCodemodeTool({
      tools: [createWriteTool(cwd)],
      artifactDirectory: join(cwd, 'output'),
      maxToolCalls: 1,
    });
    const code =
      'await tools.write({path:"completed", content:"kept"});' +
      (failure === 'exception'
        ? 'throw new Error("fixture VM failure");'
        : 'await tools.write({path:"over-budget", content:"not allowed"});');
    const native = await NativeOpenAI.create([
      calls(call('codemode', { code })),
      answer('recovered from VM error'),
    ]);
    const { execution } = startMachine(createMachine(native.bindings(cwd, { tools: [codemode] })));
    execution.send({ type: 'PROMPT', text: 'handle VM errors' });
    await ready(execution, 2);
    const result = results(native.request(1))[0];
    expect(result).toMatchObject({
      isError: true,
      nestedCalls: { calls: [{ name: 'write', status: 'ok' }] },
    });
    expect(text(result)).toContain(
      failure === 'exception' ? 'fixture VM failure' : 'nested tool call limit reached',
    );
    expect(await readFile(join(cwd, 'completed'), 'utf8')).toBe('kept');
    await expect(readFile(join(cwd, 'over-budget'))).rejects.toMatchObject({ code: 'ENOENT' });
  },
);
