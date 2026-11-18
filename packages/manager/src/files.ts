import { randomUUID } from 'node:crypto';
import { lstat, open, rename, rm } from 'node:fs/promises';
import { join, relative, resolve, sep } from 'node:path';
import { contained } from './containment.ts';
import { SourceCopy } from './source-copy.ts';
import { TreeDigest } from './tree-digest.ts';
import type { DigestOptions } from './tree-digest.ts';

export { contained, relativePath } from './containment.ts';
export type { DigestOptions } from './tree-digest.ts';

export const exists = async (path: string): Promise<boolean> => {
  try {
    await lstat(path);
    return true;
  } catch (error) {
    if (error instanceof Error && 'code' in error && error.code === 'ENOENT') {
      return false;
    }
    throw error;
  }
};
/** Check every existing path component, not just the leaf (including dangling symlinks). */
export const safePath = async (root: string, path: string): Promise<void> => {
  if (!contained(root, path)) {
    throw new Error(`Path escapes manager ownership: ${path}`);
  }
  let current = resolve(root);
  for (const segment of ['', ...relative(root, path).split(sep).filter(Boolean)]) {
    current = join(current, segment);
    if (await exists(current)) {
      const info = await lstat(current);
      if (info.isSymbolicLink()) {
        throw new Error(`Symlink in managed path: ${current}`);
      }
      if (current !== path && !info.isDirectory()) {
        throw new Error(`Non-directory in managed path: ${current}`);
      }
    }
  }
};

export const atomicJson = async (path: string, value: unknown): Promise<void> => {
  const temporary = `${path}.${randomUUID()}.tmp`;
  let published = false;
  try {
    const file = await open(temporary, 'wx', 0o600);
    try {
      await file.writeFile(`${JSON.stringify(value, null, 2)}\n`);
      await file.sync();
    } finally {
      await file.close();
    }
    await rename(temporary, path);
    published = true;
  } finally {
    if (!published) {
      await rm(temporary, { force: true });
    }
  }
};

export const copySource = async (input: {
  from: string;
  to: string;
  excludedRoot?: string;
}): Promise<void> => new SourceCopy(input.excludedRoot).copy(input.from, input.to);

export const digestTree = async (options: DigestOptions): Promise<string> =>
  new TreeDigest(options).compute();
