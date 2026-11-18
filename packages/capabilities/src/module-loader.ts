import { createHash } from 'node:crypto';
import type { Hash } from 'node:crypto';
import { readdir, readFile, realpath } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import type { Loader, MachineSource } from '@zf/core';
import { GuardedImport } from './guarded-import.ts';

export class TypeScriptModuleLoader implements Loader {
  readonly #sharedModules: Record<string, unknown>;

  public constructor(sharedModules: Record<string, unknown> = {}) {
    this.#sharedModules = sharedModules;
  }

  public async load(path: string, directory?: string): Promise<MachineSource> {
    const entry = await realpath(path);
    const root = await realpath(directory ?? dirname(entry));
    const before = await this.#revision(root);
    const guardedImport = new GuardedImport(this.#sharedModules);
    const module = await guardedImport.module(entry);
    const after = await this.#revision(root);
    if (before !== after) {
      throw new Error('Machine changed while loading');
    }
    const revision = createHash('sha256').update(after);
    await guardedImport.hashClosure(revision);
    return { path: entry, revision: revision.digest('hex'), module };
  }

  async #revision(directory: string): Promise<string> {
    const hash = createHash('sha256');
    await this.#hashFiles(hash, directory, '');
    return hash.digest('hex');
  }

  async #hashFiles(hash: Hash, directory: string, relative: string): Promise<void> {
    const entries = (await readdir(directory, { withFileTypes: true })).sort((a, b) =>
      a.name.localeCompare(b.name),
    );
    for (const entry of entries) {
      if (entry.name === 'node_modules' || entry.name === '.git') {
        continue;
      }
      if (entry.isSymbolicLink()) {
        throw new Error('Machine directory symlinks are outside the supported profile');
      }
      const name = join(relative, entry.name);
      if (entry.isDirectory()) {
        await this.#hashFiles(hash, join(directory, entry.name), name);
      } else if (entry.isFile()) {
        hash
          .update(name)
          .update('\0')
          .update(await readFile(join(directory, entry.name)))
          .update('\0');
      }
    }
  }
}
