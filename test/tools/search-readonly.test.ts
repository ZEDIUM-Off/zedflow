import { readdir } from 'node:fs/promises';
import { expect, test } from 'vite-plus/test';
import {
  assertNativeSearchParity,
  publicSearchFixture,
} from '../support/tools/search-public-process.ts';
import {
  prepareReadonlySearch,
  searchStorage,
  searchTree,
} from '../support/tools/native-search-readonly.ts';

test.each(
  ['usable', 'dev-null', 'file', 'absent', 'readonly'].flatMap((storage) =>
    [
      'path',
      'cache-only',
      'two-valid',
      'directory-cache',
      'broken-cache',
      'invalid-path',
      'missing',
      'fdfind',
      'fdfind-before-fd',
      'fdfind-invalid-sibling',
      'helper',
    ].map((scenario) => ({ storage, scenario })),
  ),
)(
  'read-only search matches public offline Pi without setup writes: $storage / $scenario',
  async ({ storage, scenario }) => {
    const box = await publicSearchFixture();
    const env = await prepareReadonlySearch(box, scenario);
    env['TMPDIR'] = await searchStorage(box, storage);
    const before = await searchTree(box);
    const cache = await readdir(box.bin);
    await assertNativeSearchParity(box, env);
    expect(await searchTree(box)).toEqual(before);
    expect(await readdir(box.bin)).toEqual(cache);
    expect(await readdir(box.tmp)).toEqual([]);
    if (scenario === 'path' || scenario === 'helper' || scenario === 'fdfind') {
      expect(await box.launches()).toEqual(['path:rg', 'path:fd', 'path:rg', 'path:fd']);
    }
    if (scenario === 'two-valid' || scenario === 'cache-only') {
      expect(await box.launches()).toEqual(['cache:rg', 'cache:fd', 'cache:rg', 'cache:fd']);
    }
    if (scenario === 'fdfind-before-fd') {
      expect(await box.launches()).toEqual(['path:rg', 'later:fd', 'path:rg', 'later:fd']);
    }
    if (scenario === 'fdfind-invalid-sibling') {
      expect(await box.launches()).toEqual(['path:rg', 'invalid:fd', 'path:rg', 'invalid:fd']);
    }
  },
);
