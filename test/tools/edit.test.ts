import { readFile, symlink, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { createWriteTool } from '../../packages/tools/src/write.ts';
import { createEditTool } from '../../packages/tools/src/edit.ts';
import { fixture, invoke } from '../support/tools/invocation.ts';

test('edit copies Pi multi-edit, fuzzy matching, CRLF and BOM preservation with diff details', async () => {
  const cwd = await fixture();
  const path = join(cwd, 'source.txt');
  await writeFile(path, '\uFEFFone\r\ntwo\r\nthree\r\nfour\r\n');
  const result = await invoke(createEditTool(cwd), {
    path: 'source.txt',
    edits: [
      { oldText: 'one\ntwo', newText: 'ONE\nTWO' },
      { oldText: 'four', newText: 'FOUR' },
    ],
  });
  expect(result.isError).toBe(false);
  expect(await readFile(path, 'utf8')).toBe('\uFEFFONE\r\nTWO\r\nthree\r\nFOUR\r\n');
  expect(result.details).toMatchObject({ firstChangedLine: 1 });
  expect(JSON.stringify(result.details)).toContain('patch');
  await writeFile(path, 'const x = “hello”;\n');
  const fuzzy = await invoke(createEditTool(cwd), {
    path: 'source.txt',
    edits: [{ oldText: 'const x = "hello";', newText: 'const x = "hi";' }],
  });
  expect(fuzzy.isError).toBe(false);
  expect(await readFile(path, 'utf8')).toBe('const x = "hi";\n');
});

test.each(
  [
    [{ oldText: 'repeat', newText: 'new' }],
    [
      { oldText: 'repeat\nunique', newText: 'x' },
      { oldText: 'unique', newText: 'y' },
    ],
    [{ oldText: '', newText: 'x' }],
    [{ oldText: 'absent', newText: 'x' }],
  ].map((edits) => ({ edits })),
)(
  'edit rejects ambiguous, overlapping, empty or missing matches without mutation: %j',
  async ({ edits }) => {
    const cwd = await fixture();
    const original = 'repeat\nunique\nrepeat\n';
    await writeFile(join(cwd, 'target'), original);
    const result = await invoke(createEditTool(cwd), { path: 'target', edits });
    expect(result.isError).toBe(true);
    expect(await readFile(join(cwd, 'target'), 'utf8')).toBe(original);
  },
);

test('edit preserves upstream argument repair for legacy and JSON-encoded edits', async () => {
  const cwd = await fixture();
  await writeFile(join(cwd, 'target'), 'one');
  expect(
    (await invoke(createEditTool(cwd), { path: 'target', oldText: 'one', newText: 'two' })).isError,
  ).toBe(false);
  expect(
    (
      await invoke(createEditTool(cwd), {
        path: 'target',
        edits: JSON.stringify([{ oldText: 'two', newText: 'three' }]),
      })
    ).isError,
  ).toBe(false);
  expect(await readFile(join(cwd, 'target'), 'utf8')).toBe('three');
});

test('mutation queue serializes symlink aliases and retains lock until aborted IO settles', async () => {
  const cwd = await fixture();
  await writeFile(join(cwd, 'target'), 'initial');
  await symlink(join(cwd, 'target'), join(cwd, 'alias'));
  const controller = new AbortController();
  let release: (() => void) | undefined;
  const gate = new Promise<void>((resolveGate) => {
    release = resolveGate;
  });
  let started: (() => void) | undefined;
  const ready = new Promise<void>((resolveReady) => {
    started = resolveReady;
  });
  const operations = {
    mkdir: async () => {},
    writeFile: async (path: string, content: string) => {
      if (content === 'first') {
        started?.();
        await gate;
      }
      await writeFile(path, content);
    },
  };
  const first = invoke(
    createWriteTool(cwd, { operations }),
    { path: 'target', content: 'first' },
    controller.signal,
  );
  await ready;
  const second = invoke(createWriteTool(cwd, { operations }), { path: 'alias', content: 'second' });
  controller.abort();
  release?.();
  expect((await first).isError).toBe(true);
  expect((await second).isError).toBe(false);
  expect(await readFile(join(cwd, 'target'), 'utf8')).toBe('second');
});

test('edit argument repair does not mutate the assistant tool call', async () => {
  const cwd = await fixture();
  await writeFile(join(cwd, 'text'), 'old');
  const args = { path: 'text', edits: JSON.stringify([{ oldText: 'old', newText: 'new' }]) };
  const before = structuredClone(args);
  const result = await invoke(createEditTool(cwd), args);
  expect(result.isError).toBe(false);
  expect(args).toEqual(before);
  expect(await readFile(join(cwd, 'text'), 'utf8')).toBe('new');
});
