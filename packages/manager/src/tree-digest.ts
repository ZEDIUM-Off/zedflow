import { createHash } from 'node:crypto';
import type { Hash } from 'node:crypto';
import { lstat, readFile, readdir, readlink, realpath } from 'node:fs/promises';
import type { Stats } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { contained } from './containment.ts';

export interface DigestOptions {
  readonly root: string;
  /** Dependency links must resolve inside the immutable snapshot; installed links are exact. */
  readonly links?: 'dependencies' | Readonly<Record<string, string>>;
}

/** pnpm's timestamps/cache bookkeeping are not runtime content; the dependency lock is separate. */
const PNPM_BOOKKEEPING = ['.modules.yaml', '.pnpm-workspace-state-v1.json'];

/**
 * One digest of one tree. It owns the running hash, so the digest covers entries in the order the
 * walk feeds them, and the link policy that decides which symlinks may enter the hash.
 */
export class TreeDigest {
  readonly #root: string;
  readonly #links: DigestOptions['links'];
  readonly #hash: Hash = createHash('sha256');

  public constructor(options: DigestOptions) {
    this.#root = options.root;
    this.#links = options.links;
  }

  public async compute(): Promise<string> {
    await this.#visit(this.#root);
    return `sha256:${this.#hash.digest('hex')}`;
  }

  async #visit(path: string): Promise<void> {
    const name = relative(this.#root, path).split(sep).join('/');
    if (this.#links === 'dependencies' && PNPM_BOOKKEEPING.includes(name)) {
      return;
    }
    const info = await lstat(path);
    if (info.isSymbolicLink()) {
      await this.#addLink(path, name);
    } else if (info.isDirectory()) {
      await this.#addDirectory(path, name);
    } else if (info.isFile()) {
      await this.#addFile(path, name, info);
    } else {
      throw new Error(`Unsupported file type: ${path}`);
    }
  }

  async #addLink(path: string, name: string): Promise<void> {
    const target = await readlink(path);
    if (this.#links === 'dependencies') {
      if (!contained(this.#root, await realpath(path))) {
        throw new Error(`Dependency link escapes its snapshot: ${path}`);
      }
    } else if (this.#links?.[name] !== target) {
      throw new Error(`Unexpected installed symlink: ${path}`);
    }
    this.#hash.update(JSON.stringify(['link', name, target]));
  }

  async #addDirectory(path: string, name: string): Promise<void> {
    this.#hash.update(JSON.stringify(['directory', name]));
    for (const child of (await readdir(path)).sort()) {
      await this.#visit(join(path, child));
    }
  }

  async #addFile(path: string, name: string, info: Stats): Promise<void> {
    // Git tracks executability, not umask-derived read/write permissions.
    const mode = info.mode & 0o111 ? 0o755 : 0o644;
    this.#hash.update(JSON.stringify(['file', name, mode, info.size]));
    this.#hash.update(await readFile(path));
  }
}
