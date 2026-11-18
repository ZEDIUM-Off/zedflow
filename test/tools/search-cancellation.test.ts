import { execFile } from 'node:child_process';
import { resolve } from 'node:path';
import { promisify } from 'node:util';
import { expect, test } from 'vite-plus/test';
import { nativeSearchBinary, nativeSearchFixture } from '../support/tools/native-search-fixture.ts';

// POSIX executable fixtures exercise native child execution, not a large-input timing race.
test.each(['find', 'grep'] as const)(
  'public %s cancellation with a non-cloneable reason exits its child and keeps the host usable',
  async (name) => {
    const rg = await nativeSearchBinary('rg');
    const fd = await nativeSearchBinary('fd');
    const box = await nativeSearchFixture({ rg, fd });
    await box.install(box.path, 'path');
    try {
      const { stdout, stderr } = await promisify(execFile)(
        process.execPath,
        [resolve('test/support/tools/native-search-cancellation-host.ts'), name, box.root, rg, fd],
        { env: box.environment(), timeout: 12000 },
      );
      expect(stderr).toBe('');
      const evidence: unknown = JSON.parse(stdout);
      expect(evidence).toMatchObject({ name, childGone: true, hostUsable: true });
    } finally {
      await box.releaseBlockingSearch();
    }
  },
);
