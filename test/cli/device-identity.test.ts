import { chmod, readdir, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { DeviceIdentity } from '../../packages/cli/src/device-identity.ts';
import { fixture } from '../support/cli/fixture.ts';
import { execute } from '../support/cli/process.ts';
import { guardNetwork } from '../support/cli/network-guard.ts';

guardNetwork();

test('concurrent device identity publication never exposes a partial UUID', async () => {
  const f = await fixture();
  const code = `import {DeviceIdentity} from ${JSON.stringify(resolve('packages/cli/src/device-identity.ts'))};console.log(new DeviceIdentity(${JSON.stringify(f.globalDirectory)}).get())`;
  const results = await Promise.all(
    Array.from({ length: 4 }, () =>
      execute(process.execPath, ['--input-type=module', '-e', code], {
        env: { PATH: process.env['PATH'], HOME: f.home, USERPROFILE: f.home, TMPDIR: f.cwd },
        timeout: 10000,
      }),
    ),
  );
  expect(new Set(results.map((result) => result.stdout.trim())).size).toBe(1);
  expect(results[0]?.stdout.trim()).toBe(new DeviceIdentity(f.globalDirectory).get());
  expect(await readdir(f.globalDirectory)).toEqual(['device-id']);
});

test('existing device identity works without directory writes and rejects a special file without blocking', async () => {
  const f = await fixture();
  const identity = new DeviceIdentity(f.globalDirectory);
  const id = identity.get();
  await chmod(f.globalDirectory, 0o500);
  try {
    expect(identity.get()).toBe(id);
  } finally {
    await chmod(f.globalDirectory, 0o700);
  }
  await rm(join(f.globalDirectory, 'device-id'));
  await execute('mkfifo', [join(f.globalDirectory, 'device-id')]);
  expect(() => identity.get()).toThrow('Invalid installation identity');
  expect(await readdir(f.globalDirectory)).toEqual(['device-id']);
});
