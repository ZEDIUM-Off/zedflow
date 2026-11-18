import { readFile, readdir } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { publicSearchFixture, runPublicSearch } from '../support/tools/search-public-process.ts';

test.each(['inline', 'split', 'commonjs', 'node-options', 'preload', 'preload-error'])(
  'public native search preserves Node input modes and inherited preloads: %s',
  async (mode) => {
    const box = await publicSearchFixture();
    await box.install(box.path, 'path');
    const env = {
      ...box.environment(),
      NODE_OPTIONS: mode === 'node-options' ? '--input-type=module' : '',
    };
    const flags =
      mode === 'split'
        ? ['--input-type', 'module']
        : [mode === 'commonjs' ? '--input-type=commonjs' : '--input-type=module'];
    const marker = join(box.root, 'preloads');
    if (mode.startsWith('preload')) {
      flags.push('--import', resolve('test/support/tools/search-entry-preload.ts'));
      Object.assign(env, {
        SEARCH_PRELOAD_MARKER: marker,
        SEARCH_PRELOAD_FAIL: mode === 'preload-error' ? '1' : '0',
      });
    }
    const result = await runPublicSearch(box, mode === 'node-options' ? [] : flags, env);
    const success = [
      { error: false, text: 'notes.txt:2: needle\nnotes.txt:4: needle' },
      { error: false, text: 'notes.txt' },
    ];
    const failure = Array.from({ length: 2 }, () => ({
      error: true,
      text: 'Search entry preload fixture',
    }));
    expect(result).toEqual({
      results: mode === 'preload-error' ? failure : success,
      fetches: 0,
      unchanged: true,
    });
    expect((await box.launches()).sort()).toEqual(
      mode === 'preload-error' ? [] : ['path:fd', 'path:rg'],
    );
    expect(await readdir(box.tmp)).toEqual([]);
    expect(await readdir(box.bin)).toEqual([]);
    if (mode.startsWith('preload')) {
      expect(await readFile(marker, 'utf8')).toBe('worker preload\nworker preload\n');
    }
  },
);
