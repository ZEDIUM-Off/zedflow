import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import { expect, test } from 'vite-plus/test';
import { createCodemodeTool } from '../../packages/tools/src/codemode.ts';
import { createBuiltinTools } from '../../packages/tools/src/index.ts';
import { createReadTool } from '../../packages/tools/src/read.ts';
import { createWriteTool } from '../../packages/tools/src/write.ts';
import { fixture, execute, text } from '../support/codemode/invocation.ts';
import { customTool } from '../support/codemode/custom-tool.ts';

test('Code Mode runs actual Pi QuickJS worker with selected built-ins, parallel calls and structured results', async () => {
  const cwd = await fixture();
  const updates: ToolResultMessage[] = [];
  const tool = createCodemodeTool({
    tools: createBuiltinTools({ cwd }),
    artifactDirectory: join(cwd, 'output'),
  });
  const result = await execute(
    tool,
    `
    await tools.write({path: "one", content: "first"});
    await tools.write({path: "two", content: "second"});
    text(await Promise.all([tools.read({path: "one"}), tools.read({path: "two"})]));
    const shell = await tools.bash({command: "printf shell; exit 3"});
    text({code: shell.exit_code, output: shell.output});
    console.log("from console");
    return ALL_TOOLS.map(t => t.name).includes("edit");
  `,
    new AbortController().signal,
    (update) => updates.push(update),
  );
  expect(result).toMatchObject({
    role: 'toolResult',
    toolCallId: 'script',
    toolName: 'codemode',
    isError: false,
  });
  expect(text(result)).toContain('["first","second"]');
  expect(text(result)).toContain('"code":3');
  expect(text(result)).toContain('==> text 1/3 <==');
  expect(text(result)).toContain('<console_output>\nfrom console\n</console_output>');
  expect(text(result)).toContain('true');
  expect(updates.length).toBeGreaterThan(5);
  expect(result.nestedCalls).toMatchObject({
    calls: [
      { name: 'write', status: 'ok' },
      { name: 'write', status: 'ok' },
      { name: 'read', status: 'ok' },
      { name: 'read', status: 'ok' },
      { name: 'bash', status: 'error' },
    ],
  });
});

test('Code Mode exposes selected capabilities only and cannot recurse or use Node/network globals', async () => {
  const cwd = await fixture();
  const tool = createCodemodeTool({
    tools: [createReadTool(cwd)],
    artifactDirectory: join(cwd, 'output'),
  });
  for (const code of [
    'await tools.write({path:"bad",content:"bad"});',
    'await tools.codemode({code:"return 1"});',
    'return process.env;',
    'return require("node:fs");',
    'return fetch("https://invalid.example");',
  ]) {
    const result = await execute(tool, code);
    expect(result.isError).toBe(true);
    expect(result.nestedCalls).toMatchObject({ calls: [] });
  }
});

test('Code Mode keeps partial output and completed mutations on script failure', async () => {
  const cwd = await fixture();
  const tool = createCodemodeTool({ tools: [createWriteTool(cwd), createReadTool(cwd)] });
  const result = await execute(
    tool,
    'await tools.write({path:"kept",content:"committed"}); text("partial"); await tools.read({path:"missing"});',
  );
  expect(result.isError).toBe(true);
  expect(text(result)).toContain('partial');
  expect(text(result)).toContain('not undone');
  expect(result.nestedCalls).toMatchObject({
    calls: [
      { name: 'write', status: 'ok' },
      { name: 'read', status: 'error' },
    ],
  });
  expect(await readFile(join(cwd, 'kept'), 'utf8')).toBe('committed');
});

test('Code Mode captures thrown tool errors, syntax errors, exit and JSON return values', async () => {
  const tool = createCodemodeTool({
    tools: [
      customTool(async () => {
        throw new Error('host tool failed');
      }),
    ],
  });
  expect(text(await execute(tool, 'await tools.custom({value:"x"});'))).toContain(
    'host tool failed',
  );
  expect((await execute(tool, 'const = ;')).isError).toBe(true);
  expect(
    (await execute(tool, 'text("before exit"); exit(); throw new Error("unreachable");')).isError,
  ).toBe(false);
  expect(text(await execute(tool, 'return {answer:42};'))).toContain('{"answer":42}');
});

test('Code Mode store/load is execution-local rather than a claim of session-harness parity', async () => {
  const tool = createCodemodeTool({ tools: [] });
  expect(text(await execute(tool, 'store("key", 42); return load("key");'))).toContain('42');
  expect(text(await execute(tool, 'return load("key") === undefined;'))).toContain('true');
  expect(tool.declaration.description).toContain('not persisted');
});

test('Code Mode and built-ins own hard-private state and share prototype execute methods', async () => {
  const cwd = await fixture();
  for (const tool of [createCodemodeTool({ tools: [] }), createReadTool(cwd)]) {
    expect(Object.keys(tool)).toEqual(['declaration']);
    expect(Object.hasOwn(tool, 'execute')).toBe(false);
    const prototype = Object.getPrototypeOf(tool) as { execute?: unknown };
    expect(typeof prototype.execute).toBe('function');
    expect(tool.declaration).toBe(tool.declaration);
  }
  const tool = createCodemodeTool({ tools: [] });
  const declaration = tool.declaration;
  expect((await execute(tool, 'return 1;')).isError).toBe(false);
  expect(tool.declaration).toBe(declaration);
  expect(tool.execute).toBe(createCodemodeTool({ tools: [] }).execute);
});
