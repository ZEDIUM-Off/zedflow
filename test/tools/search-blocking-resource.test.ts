import { execFile } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { expect, test, vi } from 'vite-plus/test';
import { nativeSearchFixture } from '../support/tools/native-search-fixture.ts';

const blockerEntry = resolve('test/support/tools/native-search-blocker.ts');
const blockingFixture = () => nativeSearchFixture({ rg: process.execPath, fd: process.execPath });

test.each(['success', 'failure'])(
  'native search cleanup ignores a completed child marker after host %s without signalling',
  async (outcome) => {
    const box = await blockingFixture();
    const marker = join(box.root, 'fd.pid');
    const blocker = promisify(execFile)(process.execPath, [
      blockerEntry,
      marker,
      box.blockingResource,
    ]);
    try {
      await expect
        .poll(() => readFile(marker, 'utf8').catch(() => ''))
        .toBe(String(blocker.child.pid));
      await box.releaseBlockingSearch();
      expect((await blocker).stderr).toBe('');
      expect(blocker.child.exitCode).toBe(0);
      expect(await readFile(marker, 'utf8')).toBe(String(blocker.child.pid));
      const signal = vi.spyOn(process, 'kill').mockImplementation(() => {
        throw new Error('Cleanup must not signal a PID');
      });
      try {
        const host = async () => {
          try {
            await promisify(execFile)(process.execPath, [
              '-e',
              `process.exitCode = ${outcome === 'failure' ? 1 : 0}`,
            ]);
          } finally {
            await box.releaseBlockingSearch();
          }
        };
        if (outcome === 'failure') {
          await expect(host()).rejects.toThrow();
        } else {
          await host();
        }
        expect(signal).not.toHaveBeenCalled();
      } finally {
        signal.mockRestore();
      }
    } finally {
      await box.releaseBlockingSearch();
      await blocker;
    }
  },
);

test('native search cleanup retires its surviving owned blocker after host failure', async () => {
  const box = await blockingFixture();
  const marker = join(box.root, 'fd.pid');
  const blocker = promisify(execFile)(process.execPath, [
    blockerEntry,
    marker,
    box.blockingResource,
  ]);
  try {
    await expect
      .poll(() => readFile(marker, 'utf8').catch(() => ''))
      .toBe(String(blocker.child.pid));
    const host = async () => {
      try {
        await promisify(execFile)(process.execPath, ['-e', 'process.exitCode = 1']);
      } finally {
        expect(blocker.child.exitCode).toBeNull();
        await box.releaseBlockingSearch();
      }
    };
    await expect(host()).rejects.toThrow();
    await expect(blocker).resolves.toMatchObject({ stdout: '', stderr: '' });
    expect(blocker.child.exitCode).toBe(0);
    expect(blocker.child.signalCode).toBeNull();
  } finally {
    await box.releaseBlockingSearch();
    await blocker;
  }
});

test('a released native search resource cannot keep a late blocker alive', async () => {
  const box = await blockingFixture();
  const marker = join(box.root, 'fd.pid');
  await box.releaseBlockingSearch();
  const blocker = promisify(execFile)(process.execPath, [
    blockerEntry,
    marker,
    box.blockingResource,
  ]);
  await expect(blocker).resolves.toMatchObject({ stdout: '', stderr: '' });
  expect(blocker.child.exitCode).toBe(0);
  await expect(readFile(marker, 'utf8')).rejects.toMatchObject({ code: 'ENOENT' });
});
