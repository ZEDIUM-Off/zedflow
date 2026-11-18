import { execFile } from 'node:child_process';
import { mkdir, mkdtemp, readFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { expect, test } from 'vite-plus/test';

const execute = promisify(execFile);
const traceHook = resolve('test/support/module-trace.ts');

// Runs node with the resolve hook inside an isolated home and returns every resolved module URL.
const traceModules = async (
  nodeArguments: (isolated: { directory: string; home: string }) => readonly string[],
  cwd = process.cwd(),
) => {
  const root = resolve('.artifacts/startup-tests');
  await mkdir(root, { recursive: true });
  const directory = await mkdtemp(join(root, 'case-'));
  const home = join(directory, 'home');
  await mkdir(home);
  const trace = join(directory, 'modules.txt');
  const result = await execute(
    process.execPath,
    ['--import', traceHook, ...nodeArguments({ directory, home })],
    {
      cwd,
      env: {
        PATH: process.env['PATH'],
        HOME: home,
        USERPROFILE: home,
        TMPDIR: directory,
        ZF_TRACE: trace,
      },
      timeout: 15000,
    },
  );
  return { stdout: result.stdout, urls: (await readFile(trace, 'utf8')).split('\n') };
};
const piModules = (urls: readonly string[]) =>
  urls.filter((url) => url.includes('pi-coding-agent'));

test.each(['--help', 'machines', 'models', 'config show agent', 'auth status'])(
  'zf %s resolves no pi-coding-agent module',
  async (command) => {
    const { urls } = await traceModules(({ directory, home }) => [
      resolve('packages/cli/bin/zf.ts'),
      '--cwd',
      directory,
      '--global-dir',
      join(home, '.zedflow'),
      ...command.split(' '),
    ]);
    expect(urls.some((url) => url.endsWith('/packages/cli/src/command.ts'))).toBe(true);
    expect(piModules(urls)).toEqual([]);
  },
);

test('importing @zf/tools from the CLI package resolves pi-coding-agent', async () => {
  const { urls } = await traceModules(
    () => ['--input-type=module', '-e', "await import('@zf/tools');"],
    resolve('packages/cli'),
  );
  expect(piModules(urls).length).toBeGreaterThan(0);
});
