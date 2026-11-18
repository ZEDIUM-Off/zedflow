import { lstat, readFile, readdir, realpath } from 'node:fs/promises';
import { isAbsolute, join, relative, resolve, sep } from 'node:path';

export interface DeclarationFile {
  readonly id: string;
  readonly directory: string;
  readonly path: string;
}
export const isMissing = (error: unknown): boolean =>
  error instanceof Error && 'code' in error && error.code === 'ENOENT';
export const validResourceId = (id: string): boolean =>
  /^[a-z0-9]+(?:-[a-z0-9]+)*(?:\/[a-z0-9]+(?:-[a-z0-9]+)*)*$/.test(id) &&
  !id.split('/').some((part) => ['constructor', 'prototype', '__proto__'].includes(part));
export const objectValue = (value: unknown): Record<string, unknown> => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('Expected an object');
  }
  return value as Record<string, unknown>;
};
export const readDeclaration = async (path: string): Promise<Record<string, unknown>> => {
  const info = await lstat(path);
  if (!info.isFile() || info.size > 65536) {
    throw new Error(`Declaration must be a regular file of at most 64 KiB: ${path}`);
  }
  try {
    return objectValue(JSON.parse((await readFile(path, 'utf8')).replace(/^\uFEFF/, '')));
  } catch {
    throw new Error(`Invalid declaration JSON: ${path}`);
  }
};
const containedRelativePath = (entry: string): boolean =>
  entry !== '' &&
  !isAbsolute(entry) &&
  !entry.includes('\\') &&
  !/^[a-z]:/i.test(entry) &&
  !entry.split('/').some((part) => !part || part === '..');
export const confinedEntry = async (directory: string, entry: string): Promise<string> => {
  if (!containedRelativePath(entry)) {
    throw new Error('Entry must be a contained relative path');
  }
  let current = directory;
  for (const part of entry.split('/').filter((part) => part !== '.')) {
    current = join(current, part);
    if ((await lstat(current)).isSymbolicLink()) {
      throw new Error('Entry symlinks are not supported');
    }
  }
  const path = await realpath(current);
  const relation = relative(await realpath(directory), path);
  if (
    !relation ||
    relation.startsWith('..' + sep) ||
    isAbsolute(relation) ||
    !(await lstat(path)).isFile()
  ) {
    throw new Error('Entry escapes its resource directory or is not a file');
  }
  return path;
};

const pathExists = async (path: string): Promise<boolean> => {
  try {
    await lstat(path);
    return true;
  } catch (error) {
    if (!isMissing(error)) {
      throw error;
    }
    return false;
  }
};

/**
 * One discovery of the declaration files named `filename` below a root, in sorted path order.
 * A directory holding a declaration is a resource leaf: its private source closure is not
 * rediscovered. Symlinked resource directories are rejected. Use one instance per discovery.
 */
class DeclarationWalk {
  readonly #filename: string;
  readonly #found: DeclarationFile[] = [];

  public constructor(filename: string) {
    this.#filename = filename;
  }

  public async files(root: string): Promise<DeclarationFile[]> {
    try {
      await lstat(root);
    } catch (error) {
      if (isMissing(error)) {
        return this.#found;
      }
      throw error;
    }
    await this.#visit(resolve(root), '');
    return this.#found;
  }

  async #visit(directory: string, id: string): Promise<void> {
    const info = await lstat(directory);
    if (!info.isDirectory() || info.isSymbolicLink()) {
      throw new Error(`Resource directories cannot be symlinks: ${directory}`);
    }
    const path = join(directory, this.#filename);
    if (!(await pathExists(path))) {
      await this.#visitChildren(directory, id);
      return;
    }
    if (!validResourceId(id)) {
      throw new Error(`Invalid resource ID: ${id}`);
    }
    this.#found.push({ id, directory: await realpath(directory), path });
  }

  async #visitChildren(directory: string, id: string): Promise<void> {
    for (const child of (await readdir(directory, { withFileTypes: true })).sort((a, b) =>
      a.name.localeCompare(b.name),
    )) {
      if (child.name.startsWith('.') || child.name === 'node_modules') {
        continue;
      }
      if (child.isSymbolicLink()) {
        throw new Error(`Resource symlinks are not supported: ${join(directory, child.name)}`);
      }
      if (child.isDirectory()) {
        await this.#visit(join(directory, child.name), id ? `${id}/${child.name}` : child.name);
      }
    }
  }
}

export const discoverDeclarations = async (
  root: string,
  filename: string,
): Promise<DeclarationFile[]> => new DeclarationWalk(filename).files(root);
