import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { JsonObject } from '@earendil-works/pi-ai';
import { expect, test } from 'vite-plus/test';
import { builtinToolNames, createBuiltinTools } from '../../packages/tools/src/index.ts';
import { createReadTool } from '../../packages/tools/src/read.ts';
import { createPowerShellTool } from '../../packages/tools/src/powershell.ts';
import { fixture, invoke, text } from '../support/tools/invocation.ts';

test('all tools reject pre-cancellation without touching fixtures', async () => {
  const cwd = await fixture();
  const controller = new AbortController();
  controller.abort();
  for (const tool of createBuiltinTools({ cwd })) {
    const result = await invoke(tool, {}, controller.signal);
    expect(result.isError).toBe(true);
    expect(result.toolName).toBe(tool.declaration.name);
  }
});

test('PowerShell distinguishes unsupported platform from supported command adapter', async () => {
  const cwd = await fixture();
  if (process.platform !== 'win32') {
    const unsupported = await invoke(createPowerShellTool(cwd), { command: 'Write-Output hello' });
    expect(unsupported.isError).toBe(true);
    expect(text(unsupported)).toContain('only available on Windows');
  }
  let commandSeen = '';
  const supported = createPowerShellTool(cwd, {
    operations: {
      exec: async (command, _cwd, options) => {
        commandSeen = command;
        options.onData(Buffer.from('hello'));
        return { exitCode: 0 };
      },
    },
  });
  const result = await invoke(supported, { command: 'Write-Output hello' });
  expect(commandSeen).toBe('Write-Output hello');
  expect(result).toMatchObject({
    toolName: 'powershell',
    isError: false,
  });
});

test('tool validation keeps native Pi coercion without mutating the original call and respects argumentsError', async () => {
  const cwd = await fixture();
  await writeFile(join(cwd, 'text'), 'first\nsecond\n');
  const tool = createReadTool(cwd);
  const args: JsonObject = { path: 'text', offset: '2', limit: '1' };
  expect(text(await invoke(tool, args))).toContain('second');
  expect(args['offset']).toBe('2');
  const malformed = await tool.execute({
    call: {
      type: 'toolCall',
      id: 'bad-json',
      name: 'read',
      arguments: { path: 'text' },
      argumentsError: 'broken JSON input',
    },
    signal: new AbortController().signal,
  });
  expect(malformed.isError).toBe(true);
  expect(text(malformed)).toContain('broken JSON input');
});

test('builtinToolNames matches the names of the created built-in tools', async () => {
  const cwd = await fixture();
  expect(createBuiltinTools({ cwd }).map((tool) => tool.declaration.name)).toEqual(
    builtinToolNames(),
  );
  expect(builtinToolNames('win32')).toEqual([...builtinToolNames('linux'), 'powershell']);
});
