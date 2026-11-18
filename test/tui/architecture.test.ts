import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from 'vite-plus/test';

test('native sources use only injected terminal, models and run service: no process, browser, clipboard or provider access', async () => {
  const directory = join('packages', 'tui', 'src');
  const files = await readdir(directory);
  const forbidden = [
    'child_process',
    'node:fs',
    'node:os',
    'homedir',
    'process.env',
    'process.stdin',
    'process.stdout',
    'fetch(',
    'clipboard',
    'openBrowser',
    'setInterval',
    'Loader',
    '@zf/config',
    '@zf/capabilities',
    '@zf/storage',
    '@zf/run',
    '@zf/xstate',
    'xstate',
  ];
  for (const file of files) {
    const source = await readFile(join(directory, file), 'utf8');
    for (const word of forbidden) {
      expect(source, `${file} mentions ${word}`).not.toContain(word);
    }
    expect(source).not.toMatch(/\bany\b\s*[,>)=;]/u);
  }
});
