import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { createGrepTool } from '../../packages/tools/src/grep.ts';
import { createFindTool } from '../../packages/tools/src/find.ts';
import { createLsTool } from '../../packages/tools/src/ls.ts';
import { fixture, invoke, text } from '../support/tools/invocation.ts';

test('grep uses real ripgrep with ignore rules, context, literal matching, caps and errors', async () => {
  const cwd = await fixture();
  await writeFile(join(cwd, '.gitignore'), 'ignored.txt\n');
  await mkdir(join(cwd, '.git'));
  await writeFile(join(cwd, 'ignored.txt'), 'Needle\n');
  await writeFile(join(cwd, 'source.ts'), 'before\nNeedle[0]\nafter\nNeedle[1]\n');
  const tool = createGrepTool(cwd);
  const result = await invoke(tool, {
    pattern: 'needle[0]',
    literal: true,
    ignoreCase: true,
    glob: '*.ts',
    context: 1,
  });
  expect(result.isError).toBe(false);
  expect(text(result)).toContain('source.ts:2: Needle[0]');
  expect(text(result)).toContain('before');
  expect(text(result)).not.toContain('ignored.txt');
  const capped = await invoke(tool, { pattern: 'Needle', limit: 1 });
  expect(capped.details).toMatchObject({ matchLimitReached: 1 });
  expect((await invoke(tool, { pattern: '[' })).isError).toBe(true);
  expect(text(await invoke(tool, { pattern: 'absent' }))).toBe('No matches found');
  expect((await invoke(tool, { pattern: 'x', path: 'missing' })).isError).toBe(true);
});

test('find uses real fd glob matching and ignore rules; ls sorts with directories and reports caps', async () => {
  const cwd = await fixture();
  await mkdir(join(cwd, '.git'));
  await mkdir(join(cwd, 'nested'));
  await writeFile(join(cwd, '.gitignore'), 'ignored.ts\n');
  await writeFile(join(cwd, 'ignored.ts'), '');
  await writeFile(join(cwd, 'z.ts'), '');
  await writeFile(join(cwd, 'nested/a.ts'), '');
  const found = await invoke(createFindTool(cwd), { pattern: '**/*.ts' });
  expect(found.isError).toBe(false);
  expect(text(found)).toContain('nested/a.ts');
  expect(text(found)).not.toContain('ignored.ts');
  expect(
    (await invoke(createFindTool(cwd), { pattern: '**/*.ts', limit: 1 })).details,
  ).toMatchObject({ resultLimitReached: 1 });
  const listed = await invoke(createLsTool(cwd), {});
  expect(text(listed)).toContain('nested/');
  expect(text(listed)).toContain('.git/');
  expect((await invoke(createLsTool(cwd), { limit: 1 })).details).toMatchObject({
    entryLimitReached: 1,
  });
  expect((await invoke(createLsTool(cwd), { path: 'z.ts' })).isError).toBe(true);
});

test('grep truncates long matching lines with useful details and handles explicit file paths', async () => {
  const cwd = await fixture();
  await writeFile(join(cwd, 'long.txt'), `match ${'x'.repeat(1000)}\n`);
  const result = await invoke(createGrepTool(cwd), { path: 'long.txt', pattern: 'match' });
  expect(result.isError).toBe(false);
  expect(result.details).toMatchObject({ linesTruncated: true });
  expect(text(result)).toContain('long.txt:1: match');
  expect(text(result)).toContain('truncated');
});

test('find and ls surface operation failures and in-flight cancellation through public factories', async () => {
  const cwd = await fixture();
  const controller = new AbortController();
  const finding = createFindTool(cwd, {
    operations: {
      exists: async () => {
        controller.abort();
        return true;
      },
      glob: async () => ['never'],
    },
  });
  expect((await invoke(finding, { pattern: '*' }, controller.signal)).isError).toBe(true);
  const listing = createLsTool(cwd, {
    operations: {
      exists: () => true,
      stat: () => ({ isDirectory: () => true }),
      readdir: async () => {
        throw new Error('denied fixture');
      },
    },
  });
  expect(text(await invoke(listing, {}))).toContain('Cannot read directory: denied fixture');
  expect((await invoke(createFindTool(cwd), { path: 'missing', pattern: '*' })).isError).toBe(true);
});
