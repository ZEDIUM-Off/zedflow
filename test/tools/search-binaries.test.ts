import { access, chmod, mkdir, writeFile } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { constants } from 'node:fs';
import { join, resolve } from 'node:path';
import { expect, test, vi } from 'vite-plus/test';
import { fixture, invoke, text } from '../support/tools/invocation.ts';
import { createFindTool } from '../../packages/tools/src/find.ts';
import { missingExecutable, searchWithPath } from '../support/tools/search-path.ts';

test('grep and find fail explicitly without downloading when rg or fd is missing from PATH', async () => {
  const cwd = await fixture();
  const { grep, find, download } = await searchWithPath(cwd, '');
  expect(grep.isError).toBe(true);
  expect(text(grep)).toBe(missingExecutable('rg'));
  expect(find.isError).toBe(true);
  expect(text(find)).toBe(missingExecutable('fd'));
  expect(download).not.toHaveBeenCalled();
});

test('grep and find reject PATH candidates that pass an access check but cannot execute', async () => {
  const cwd = await fixture();
  const bin = join(cwd, 'bin');
  await mkdir(join(bin, 'rg'), { recursive: true });
  for (const name of ['fd', 'fdfind']) {
    await writeFile(join(bin, name), '#!/nonexistent/interpreter\n');
    await chmod(join(bin, name), 0o755);
  }
  for (const name of ['rg', 'fd', 'fdfind']) {
    await expect(access(join(bin, name), constants.X_OK)).resolves.toBeUndefined();
  }
  const { grep, find, download } = await searchWithPath(cwd, bin);
  expect(text(grep)).toBe(missingExecutable('rg'));
  expect(text(find)).toBe(missingExecutable('fd'));
  expect(download).not.toHaveBeenCalled();
});

test.each(['alternating', 'throwing'])(
  'find captures a custom operations getter once even when a second read is %s',
  async (secondRead) => {
    const cwd = await fixture();
    let reads = 0;
    const operations = {
      exists: vi.fn(() => true),
      glob: vi.fn(() => [join(cwd, 'found.ts')]),
    };
    const options = Object.defineProperty({}, 'operations', {
      get: () => {
        reads += 1;
        if (reads === 1) {
          return operations;
        }
        if (secondRead === 'throwing') {
          throw new Error('Duplicate operations observation');
        }
        return undefined;
      },
    });
    const download = vi.fn(() => {
      throw new Error('Unexpected download');
    });
    vi.stubGlobal('fetch', download);
    vi.stubEnv('PATH', '');
    try {
      const result = await invoke(createFindTool(cwd, options), { pattern: '*.ts' });
      expect(result.isError).toBe(false);
      expect(text(result)).toBe('found.ts');
      expect(reads).toBe(1);
      expect(operations.exists).toHaveBeenCalledExactlyOnceWith(cwd);
      expect(operations.glob).toHaveBeenCalledExactlyOnceWith('*.ts', cwd, {
        ignore: ['**/node_modules/**', '**/.git/**'],
        limit: 1001,
      });
      expect(download).not.toHaveBeenCalled();
    } finally {
      vi.unstubAllEnvs();
      vi.unstubAllGlobals();
    }
  },
);

test('find uses its captured undefined operations for the native offline error without downloading', async () => {
  const cwd = await fixture();
  let reads = 0;
  const glob = vi.fn(() => ['must-not-run']);
  const options = Object.defineProperty({}, 'operations', {
    get: () => {
      reads += 1;
      return reads === 1 ? undefined : { exists: () => true, glob };
    },
  });
  const download = vi.fn(() => {
    throw new Error('Unexpected download');
  });
  vi.stubGlobal('fetch', download);
  vi.stubEnv('PATH', '');
  vi.stubEnv('PI_CODING_AGENT_DIR', join(cwd, 'empty-agent'));
  try {
    const result = await invoke(createFindTool(cwd, options), { pattern: '*' });
    expect(result.isError).toBe(true);
    expect(text(result)).toBe(missingExecutable('fd'));
    expect(reads).toBe(1);
    expect(glob).not.toHaveBeenCalled();
    expect(download).not.toHaveBeenCalled();
  } finally {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  }
});

test('pinned native find observes custom operations once and fails offline without an executable or network', async () => {
  const cwd = await fixture();
  const script = `
    let fetches = 0;
    globalThis.fetch = async () => { fetches++; throw new Error('Unexpected network'); };
    const { createFindToolDefinition } = await import('@earendil-works/pi-coding-agent');
    let reads = 0;
    const options = { get operations() {
      if (++reads !== 1) { throw new Error('Duplicate operations observation'); }
      return { exists: () => true, glob: () => ['native.ts'] };
    } };
    const native = createFindToolDefinition(process.cwd(), options);
    const result = await native.execute('native-control', { pattern: '*.ts' }, new AbortController().signal);
    let missing;
    try {
      await createFindToolDefinition(process.cwd()).execute('native-control', { pattern: '*' }, new AbortController().signal);
    } catch (error) { missing = error.message; }
    console.log(JSON.stringify({ reads, content: result.content, missing, fetches }));
  `;
  const result = await promisify(execFile)(
    process.execPath,
    ['--input-type=module', '-e', script],
    {
      cwd: resolve('packages/tools'),
      env: {
        HOME: cwd,
        USERPROFILE: cwd,
        PI_CODING_AGENT_DIR: join(cwd, 'pi'),
        PATH: '',
        PI_OFFLINE: '1',
      },
      timeout: 10000,
    },
  );
  const observed: unknown = JSON.parse(result.stdout);
  expect(observed).toEqual({
    reads: 1,
    content: [{ type: 'text', text: 'native.ts' }],
    missing: 'fd is not available and could not be downloaded',
    fetches: 0,
  });
});
