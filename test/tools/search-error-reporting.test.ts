import { execFile } from 'node:child_process';
import { resolve } from 'node:path';
import { promisify } from 'node:util';
import { expect, test } from 'vite-plus/test';
import { nativeSearchBinary, nativeSearchFixture } from '../support/tools/native-search-fixture.ts';
import { searchErrorReasons } from '../support/tools/search-error-reason.ts';

for (const mode of ['abort', 'reject', 'pre-grep', 'pre-find']) {
  test.each(searchErrorReasons)(
    `public search error reporting ${mode} with %s keeps the same host usable`,
    async (name) => {
      const rg = await nativeSearchBinary('rg');
      const fd = await nativeSearchBinary('fd');
      const box = await nativeSearchFixture({ rg, fd });
      await box.install(box.path, 'path');
      const { stdout, stderr } = await promisify(execFile)(
        process.execPath,
        [
          resolve('test/support/tools/search-error-reporting-host.ts'),
          mode,
          name,
          box.root,
          rg,
          fd,
        ],
        { env: box.environment(), timeout: 12000 },
      );
      expect(stderr).toBe('');
      const evidence: unknown = JSON.parse(stdout);
      expect(evidence).toMatchObject({ mode, name, hostUsable: true });
    },
  );
}
