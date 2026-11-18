import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { resolve } from 'node:path';
import { nativeSearchBinary, nativeSearchFixture } from './native-search-fixture.ts';
import type { NativeSearchFixture } from './native-search-fixture.ts';

// JavaScript entry controls exercise null options and Node input modes without weakening TS APIs.
const program = `(async () => {
  const assert = (await import('node:assert/strict')).default;
  const { join } = await import('node:path');
  const { pathToFileURL } = await import('node:url');
  const root = process.argv[1], cwd = process.argv[2], mode = process.argv[3];
  const { createGrepTool, createFindTool } = await import(pathToFileURL(join(root, 'packages/tools/src/index.ts')));
  const { invoke, text } = await import(pathToFileURL(join(root, 'test/support/tools/invocation.ts')));
  const before = JSON.stringify({ args: process.execArgv, env: { ...process.env } });
  let fetches = 0;
  globalThis.fetch = async () => { fetches++; throw Error('Unexpected network'); };
  const results = [];
  if (mode === 'null-grep') {
    let reads = 0;
    const options = { get operations() { reads++; return null; } };
    const tool = createGrepTool(cwd, options), ordinary = createGrepTool(cwd);
    for (const args of [{ pattern: 'needle', path: 'absent' }, { pattern: 'needle', path: '.', glob: 'nested/*.ts' }]) {
      const a = await invoke(tool, args), b = await invoke(ordinary, args);
      const value = r => ({ error: r.isError, text: text(r), details: r.details });
      assert.deepEqual(value(a), value(b));
      results.push(value(a));
    }
    assert.equal(reads, 1);
  } else {
    for (const [tool, args] of [[createGrepTool(cwd), { pattern: 'needle', path: 'notes.txt' }],
      [createFindTool(cwd), { pattern: '*.txt' }]]) {
      const r = await invoke(tool, args);
      results.push({ error: r.isError, text: text(r), details: r.details });
    }
  }
  assert(before === JSON.stringify({ args: process.execArgv, env: { ...process.env } }), 'Caller flags/environment changed');
  console.log(JSON.stringify({ results, fetches, unchanged: true }));
})().catch(error => { console.error(error); process.exitCode = 1; });`;

// A fresh offline public-root process is the oracle; no private Pi API or selection emulation.
const oracleProgram = `
  import { join } from 'node:path';
  import { pathToFileURL } from 'node:url';
  const [root, cwd] = process.argv.slice(1);
  const before = JSON.stringify({ ...process.env });
  let fetches = 0;
  globalThis.fetch = async () => { fetches++; throw Error('Unexpected network'); };
  const pi = await import(import.meta.resolve('@earendil-works/pi-coding-agent', pathToFileURL(join(root, 'packages/tools/package.json')).href));
  const results = [];
  for (const [tool, args] of [[pi.createGrepTool(cwd), { pattern: 'needle', path: 'notes.txt' }],
    [pi.createFindTool(cwd), { pattern: '*.txt' }]]) {
    try {
      const r = await tool.execute('oracle', args, new AbortController().signal);
      results.push({ error: false, text: r.content.map(block => block.text).join('\\n'), details: r.details });
    } catch (error) { results.push({ error: true, text: error.message }); }
  }
  console.log(JSON.stringify({ results, fetches, unchanged: before === JSON.stringify({ ...process.env }) }));
`;

export const publicSearchFixture = async (): Promise<NativeSearchFixture> => {
  return nativeSearchFixture({
    rg: await nativeSearchBinary('rg'),
    fd: await nativeSearchBinary('fd'),
  });
};

export const assertNativeSearchParity = async (
  box: NativeSearchFixture,
  env: NodeJS.ProcessEnv,
): Promise<void> => {
  const before = await box.launches();
  const native = await runPublicSearch(box, [], { ...env, PI_OFFLINE: '1' }, 'oracle');
  const oracleLaunches = await box.launches();
  assert(native !== null && typeof native === 'object');
  assert('fetches' in native && native.fetches === 0);
  assert('unchanged' in native && native.unchanged === true);
  const adapted = await runPublicSearch(box, [], env);
  const adaptedLaunches = await box.launches();
  assert.deepEqual(adapted, native);
  assert.deepEqual(
    adaptedLaunches.slice(oracleLaunches.length),
    oracleLaunches.slice(before.length),
  );
};

export const runPublicSearch = async (
  box: NativeSearchFixture,
  flags: readonly string[],
  env: NodeJS.ProcessEnv,
  mode = 'ordinary',
): Promise<unknown> => {
  const result = await promisify(execFile)(
    process.execPath,
    [
      ...(mode === 'oracle'
        ? ['--experimental-import-meta-resolve', '--input-type=module']
        : flags),
      '-e',
      mode === 'oracle' ? oracleProgram : program,
      resolve('.'),
      box.cwd,
      mode,
    ],
    { cwd: box.root, env, timeout: 12000 },
  );
  if (result.stderr) {
    throw new Error(result.stderr);
  }
  const parsed: unknown = JSON.parse(result.stdout);
  return parsed;
};
