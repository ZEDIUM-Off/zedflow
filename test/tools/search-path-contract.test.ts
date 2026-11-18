import { readdir } from 'node:fs/promises';
import { delimiter } from 'node:path';
import { expect, test } from 'vite-plus/test';
import {
  assertNativeSearchParity,
  publicSearchFixture,
} from '../support/tools/search-public-process.ts';

test.each(
  ['absent', 'empty', 'empty-segments', 'dot', 'relative', 'mixed'].flatMap((path) =>
    [false, true].map((cache) => ({ path, cache })),
  ),
)(
  'public native search follows Pi PATH semantics and cache priority: $path cache=$cache',
  async ({ path, cache }) => {
    const box = await publicSearchFixture();
    await box.install(box.root, 'cwd');
    await box.install(box.path, 'explicit');
    if (cache) {
      await box.install(box.bin, 'cache');
    }
    const env: NodeJS.ProcessEnv = { ...box.environment(), NODE_OPTIONS: '', PI_OFFLINE: '0' };
    const paths: Record<string, string> = {
      empty: '',
      'empty-segments': delimiter.repeat(2),
      dot: '.',
      relative: 'path',
      mixed: `${delimiter}path${delimiter}${delimiter}`,
    };
    if (path === 'absent') {
      Reflect.deleteProperty(env, 'PATH');
    } else {
      env['PATH'] = paths[path] ?? '';
    }
    await assertNativeSearchParity(box, env);
    const selected = cache ? 'cache' : path === 'relative' ? 'explicit' : 'cwd';
    expect((await box.launches()).sort()).toEqual(
      path === 'absent' && !cache
        ? []
        : Array.from({ length: 2 }, () => [`${selected}:fd`, `${selected}:rg`])
            .flat()
            .sort(),
    );
    expect(await readdir(box.tmp)).toEqual([]);
  },
);
