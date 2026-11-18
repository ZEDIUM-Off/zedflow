import { chmod, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { delimiter, join } from 'node:path';
import { quoteShell } from './native-search-fixture.ts';
import type { NativeSearchFixture } from './native-search-fixture.ts';

const requireHelper = async (box: NativeSearchFixture): Promise<void> => {
  const helpers = join(box.root, 'helpers');
  await mkdir(helpers);
  await writeFile(join(helpers, 'required-helper'), '#!/bin/sh\nexit 0\n', { mode: 0o700 });
  for (const name of ['rg', 'fd']) {
    const target = join(box.path, name);
    await writeFile(
      target,
      (await readFile(target, 'utf8')).replace(
        'case "$*"',
        'required-helper || exit 17\ncase "$*"',
      ),
    );
  }
};

const prepareCache = async (box: NativeSearchFixture, scenario: string): Promise<void> => {
  if (['cache-only', 'two-valid'].includes(scenario)) {
    await box.install(box.bin, 'cache');
  } else if (scenario === 'directory-cache') {
    await Promise.all(['rg', 'fd'].map((name) => mkdir(join(box.bin, name))));
  } else if (scenario === 'broken-cache') {
    await box.broken(box.bin, 'exit 42');
  }
};

export const prepareReadonlySearch = async (
  box: NativeSearchFixture,
  scenario: string,
): Promise<NodeJS.ProcessEnv> => {
  const env: NodeJS.ProcessEnv = { ...box.environment(), NODE_OPTIONS: '', PI_OFFLINE: '0' };
  await prepareCache(box, scenario);
  if (scenario !== 'missing' && scenario !== 'cache-only') {
    await box.install(box.path, 'path', scenario.startsWith('fdfind'));
  }
  if (scenario === 'invalid-path') {
    await box.broken(box.path, 'exit 42');
  } else if (scenario === 'fdfind-before-fd') {
    await box.install(join(box.root, 'later'), 'later');
    env['PATH'] += delimiter + join(box.root, 'later');
  } else if (scenario === 'fdfind-invalid-sibling') {
    await writeFile(
      join(box.path, 'fd'),
      `#!/bin/sh\nif [ "$1" = --version ]; then exit 7; fi\nprintf '%s\\n' invalid:fd >> ${quoteShell(join(box.root, 'launched'))}\nexit 7\n`,
      { mode: 0o700 },
    );
  } else if (scenario === 'helper') {
    await requireHelper(box);
    env['PATH'] += delimiter + join(box.root, 'helpers');
  }
  return env;
};

export const searchStorage = async (box: NativeSearchFixture, kind: string): Promise<string> => {
  if (kind === 'usable') {
    return box.tmp;
  }
  if (kind === 'dev-null') {
    return '/dev/null';
  }
  const target = join(box.root, 'unusable-tmp');
  if (kind === 'file') {
    await writeFile(target, 'not a directory');
  } else if (kind === 'readonly') {
    await mkdir(target);
    await chmod(target, 0o555);
  }
  return target;
};

export const searchTree = async (box: NativeSearchFixture): Promise<string[]> =>
  (await readdir(box.root, { recursive: true })).filter((entry) => entry !== 'launched').sort();
