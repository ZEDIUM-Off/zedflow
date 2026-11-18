import { chmod, mkdir, readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from 'vite-plus/test';
import {
  assertNativeSearchParity,
  publicSearchFixture,
  runPublicSearch,
} from '../support/tools/search-public-process.ts';
import { missingExecutable } from '../support/tools/search-path.ts';

test.each(['nonzero', 'wrong-program', 'timeout', 'directory', 'inert', 'non-executable'])(
  'native Pi cache priority bypasses %s PATH candidates without Zedflow probing',
  async (kind) => {
    const box = await publicSearchFixture();
    const bodies: Record<string, string> = {
      nonzero: 'exit 42',
      'wrong-program': 'echo impostor',
      timeout: 'exec /bin/sleep 20',
      inert: '',
      'non-executable': 'echo ripgrep 14.1',
    };
    if (kind === 'directory') {
      await mkdir(join(box.path, 'rg'));
    } else {
      await box.broken(box.path, bodies[kind] ?? '');
    }
    if (kind === 'non-executable') {
      await chmod(join(box.path, 'rg'), 0o600);
    }
    await box.install(box.bin, 'cache');
    await assertNativeSearchParity(box, { ...box.environment(), PI_OFFLINE: '0' });
    expect((await box.launches()).sort()).toEqual(['cache:fd', 'cache:fd', 'cache:rg', 'cache:rg']);
    expect(await readdir(box.tmp)).toEqual([]);
  },
);

test('missing native resolution isolates PATH and cache and retains explicit offline errors', async () => {
  const box = await publicSearchFixture();
  await assertNativeSearchParity(box, box.environment());
  expect(await runPublicSearch(box, [], box.environment())).toEqual({
    results: [
      { error: true, text: missingExecutable('rg') },
      { error: true, text: missingExecutable('fd') },
    ],
    fetches: 0,
    unchanged: true,
  });
  expect(await box.launches()).toEqual([]);
  expect(await readdir(box.tmp)).toEqual([]);
  expect(await readdir(box.bin)).toEqual([]);
});
