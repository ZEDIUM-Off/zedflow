import { execFile } from 'node:child_process';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';

// Public JavaScript operations may lose glob or be null; exercise those values without TS casts.
const program = `(async () => {
  const assert = (await import('node:assert/strict')).default;
  const { join } = await import('node:path');
  const { pathToFileURL } = await import('node:url');
  const { setImmediate } = await import('node:timers/promises');
  const root = process.argv[1], cwd = process.argv[2], scenario = process.argv[3];
  const { createFindTool } = await import(pathToFileURL(join(root, 'packages/tools/src/index.ts')));
  const { invoke, text } = await import(pathToFileURL(join(root, 'test/support/tools/invocation.ts')));
  let fetches = 0, reads = 0, n = 0;
  globalThis.fetch = async () => { fetches++; throw Error('Unexpected download'); };
  const trace = [], ctl = new AbortController();
  let release;
  const pendingExists = new Promise(resolve => { release = resolve; });
  const ops = {
    get glob() {
      trace.push('glob.get:' + ++n);
      if (scenario === 'getter-throws') throw Error('getter failed');
      if (scenario === 'glob-aborts') ctl.abort();
      if (scenario.startsWith('getter-aborts-')) {
        ctl.abort(new Error('caller stop'));
        if (scenario === 'getter-aborts-then-throws') throw Error('late getter error');
      }
      const version = n;
      return function(pattern, path, options) {
        assert.equal(this, ops);
        assert.equal(path, cwd);
        assert.deepEqual(options, { ignore: ['**/node_modules/**', '**/.git/**'], limit: 1001 });
        trace.push(['glob.call', version, pattern]);
        return ['g' + version + '.txt'];
      };
    },
    get exists() {
      trace.push('exists.get');
      return async function(path) {
        assert.equal(this, ops); assert.equal(path, cwd);
        trace.push('exists.call');
        if (scenario === 'getter-aborts-pending-exists') {
          await pendingExists; trace.push('exists.completed');
        } else await Promise.resolve();
        if (scenario === 'changed-while-exists') Object.defineProperty(ops, 'glob', {
          configurable: true, value: function() { assert.equal(this, ops); trace.push('replacement.call'); return ['replacement.txt']; }
        });
        if (scenario === 'removed-while-exists') Object.defineProperty(ops, 'glob', { configurable: true, value: undefined });
        return scenario !== 'exists-false';
      };
    }
  };
  const originalGlob = Object.getOwnPropertyDescriptor(ops, 'glob');
  assert(originalGlob);
  const options = { get operations() { reads++; return scenario === 'null-operations' ? null : ops; } };
  const tool = createFindTool(cwd, options);
  if (scenario === 'pre-aborted') ctl.abort();
  if (scenario === 'absent-after-creation') Object.defineProperty(ops, 'glob', { configurable: true, value: undefined });
  const results = [];
  const run = async () => {
    const r = await invoke(tool, { pattern: '*.txt' }, ctl.signal);
    results.push({ error: r.isError, text: text(r).replaceAll(cwd, '<cwd>'), details: r.details });
  };
  await run();
  if (scenario.startsWith('getter-aborts-')) {
    trace.push('settled');
    assert(!trace.includes('exists.completed'));
    release();
  }
  if (scenario.startsWith('reuse')) {
    Object.defineProperty(ops, 'glob', { configurable: true, value: undefined });
    await run(); await run();
    Object.defineProperty(ops, 'glob', originalGlob); await run();
    Object.defineProperty(ops, 'glob', { configurable: true, value: null }); await run();
  }
  await setImmediate();
  console.log(JSON.stringify({ results, trace, reads, fetches }));
})().catch(error => { console.error(error); process.exitCode = 1; });`;

export const runFindPublicScenario = async (
  cwd: string,
  scenario: string,
  env?: NodeJS.ProcessEnv,
): Promise<unknown> => {
  const result = await promisify(execFile)(
    process.execPath,
    ['-e', program, resolve('.'), cwd, scenario],
    {
      env: env ?? {
        ...process.env,
        NODE_OPTIONS: '',
        PATH: join(cwd, 'no-path'),
        PI_CODING_AGENT_DIR: join(cwd, 'no-agent'),
        PI_OFFLINE: '',
        TMPDIR: join(cwd, 'no-tmp'),
      },
      timeout: 12000,
    },
  );
  if (result.stderr) {
    throw new Error(result.stderr);
  }
  const parsed: unknown = JSON.parse(result.stdout);
  return parsed;
};
