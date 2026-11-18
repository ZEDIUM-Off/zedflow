import { mkdir, readdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { publicSearchFixture, runPublicSearch } from '../support/tools/search-public-process.ts';

test('public JavaScript null grep operations preserve default missing-path and directory-glob behavior', async () => {
  const box = await publicSearchFixture();
  await box.install(box.path, 'path');
  await mkdir(join(box.cwd, 'nested'));
  await writeFile(join(box.cwd, 'nested', 'hit.ts'), 'needle\n');
  await writeFile(join(box.cwd, 'nested', 'miss.txt'), 'needle\n');
  const result = await runPublicSearch(
    box,
    [],
    { ...box.environment(), NODE_OPTIONS: '' },
    'null-grep',
  );
  expect(result).toEqual({
    results: [
      { error: true, text: `Path not found: ${join(box.cwd, 'absent')}` },
      { error: false, text: 'nested/hit.ts:1: needle' },
    ],
    fetches: 0,
    unchanged: true,
  });
  expect(await box.launches()).toEqual(['path:rg', 'path:rg']);
  expect(await readdir(box.tmp)).toEqual([]);
});
