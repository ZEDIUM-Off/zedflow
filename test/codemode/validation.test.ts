import { wrapToolDefinition } from '../../packages/tools/src/definition.ts';
import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { JsonObject } from '@earendil-works/pi-ai';
import type { ExecutableTool } from '@zf/core';
import { expect, test } from 'vite-plus/test';
import { createCodemodeTool } from '../../packages/tools/src/codemode.ts';
import { createBuiltinTools } from '../../packages/tools/src/index.ts';
import { fixture, execute, text } from '../support/codemode/invocation.ts';
import { customTool } from '../support/codemode/custom-tool.ts';

test('Code Mode validates nested calls even for tools that do not validate themselves', async () => {
  let calls = 0;
  const tool = createCodemodeTool({
    tools: [
      customTool(async () => {
        calls++;
        throw new Error('must not run');
      }),
    ],
  });
  const invalid = await execute(tool, 'await tools.custom({wrong:"input"});');
  expect(invalid.isError).toBe(true);
  expect(text(invalid)).toContain('Validation failed');
  expect(calls).toBe(0);
  expect(invalid.nestedCalls).toMatchObject({ calls: [{ name: 'custom', status: 'error' }] });
  const scalar = await execute(tool, 'await tools.custom("not an object");');
  expect(text(scalar)).toContain('arguments must be an object');
  expect(calls).toBe(0);
});

test('Code Mode preserves built-in edit repair before validation', async () => {
  const cwd = await fixture();
  await writeFile(join(cwd, 'text'), 'old');
  const result = await execute(
    createCodemodeTool({
      tools: createBuiltinTools({ cwd }),
      artifactDirectory: join(cwd, 'output'),
    }),
    'text(await tools.edit({path:"text",oldText:"old",newText:"new"}));',
  );
  expect(result.isError).toBe(false);
  expect(await readFile(join(cwd, 'text'), 'utf8')).toBe('new');
});

test('Code Mode uses native-style preparation hooks for arbitrary injected tools before validating', async () => {
  let observed = '';
  const tool: ExecutableTool = {
    ...customTool(async (args) => {
      if (typeof args['value'] !== 'string') {
        throw new Error('Expected validated string');
      }
      observed = args['value'];
      return {
        role: 'toolResult',
        toolCallId: 'custom',
        toolName: 'custom',
        content: [{ type: 'text', text: observed }],
        isError: false,
        timestamp: 0,
      };
    }),
    prepareArguments: (args) => ({ value: (args as JsonObject)['legacy'] }),
  };
  const result = await execute(
    createCodemodeTool({ tools: [tool] }),
    'text(await tools.custom({legacy:"prepared"}));',
  );
  expect(result.isError).toBe(false);
  expect(observed).toBe('prepared');
  expect(result.nestedCalls?.calls[0]?.arguments).toEqual({ legacy: 'prepared' });
  const invalid = await execute(
    createCodemodeTool({ tools: [{ ...tool, prepareArguments: () => ({ invalid: true }) }] }),
    'await tools.custom({legacy:"bad"});',
  );
  expect(invalid.isError).toBe(true);
  expect(observed).toBe('prepared');
});

test('wrapped builtin normalization runs exactly once through Code Mode', async () => {
  let preparations = 0;
  const tool = wrapToolDefinition({
    name: 'normalize',
    label: 'Normalize',
    description: 'Fixture',
    parameters: { type: 'object', properties: { value: { type: 'string' } }, required: ['value'] },
    prepareArguments: (raw: unknown) => {
      preparations++;
      return { value: (raw as JsonObject)['legacy'] };
    },
    execute: async () => ({ content: [{ type: 'text', text: 'normalized' }] }),
  });
  const result = await execute(
    createCodemodeTool({ tools: [tool] }),
    'text(await tools.normalize({legacy:"once"}));',
  );
  expect(result.isError).toBe(false);
  expect(preparations).toBe(1);
});
