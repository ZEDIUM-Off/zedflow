import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { resolve } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { nativeSearchBinary } from '../support/tools/native-search-fixture.ts';

const realBinaries = async (): Promise<string[]> =>
  Promise.all([nativeSearchBinary('rg'), nativeSearchBinary('fd')]);

test.each([
  'cache-only',
  'two-valid',
  'directory-cache',
  'bad-path-nonzero',
  'bad-path-identity',
  'fdfind',
])('native search follows Pi selection and cleans concurrent resources: %s', async (scenario) => {
  const { stdout, stderr } = await promisify(execFile)(
    process.execPath,
    [resolve('test/support/tools/native-search-selection.ts'), scenario, ...(await realBinaries())],
    { timeout: 12000 },
  );
  expect(stderr).toBe('');
  expect(JSON.parse(stdout)).toMatchObject({ scenario, fetches: 0, clean: true });
});

test.each(['cancel-grep', 'cancel-find', 'startup-error', 'pre-aborted', 'invalid'])(
  'native search owns startup, native processes and cleanup before delivery: %s',
  async (scenario) => {
    const preload =
      scenario === 'startup-error'
        ? ['--import', resolve('test/support/tools/native-search-startup-error.ts')]
        : [];
    const { stdout, stderr } = await promisify(execFile)(
      process.execPath,
      [
        ...preload,
        resolve('test/support/tools/native-search-lifecycle.ts'),
        scenario,
        ...(await realBinaries()),
      ],
      { timeout: 12000 },
    );
    expect(stderr).toBe('');
    expect(JSON.parse(stdout)).toEqual({ scenario, clean: true });
  },
);

test('native search resolves on every call, preserving relative PATH and fd alias order across search cwd', async () => {
  const { stdout, stderr } = await promisify(execFile)(
    process.execPath,
    [resolve('test/support/tools/native-search-call-time.ts'), ...(await realBinaries())],
    { timeout: 12000 },
  );
  expect(stderr).toBe('');
  expect(JSON.parse(stdout)).toEqual({ callTime: true, clean: true });
});
