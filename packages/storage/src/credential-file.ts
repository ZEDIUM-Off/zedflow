// Adapted from Pi 1.5.2 core/auth-storage.ts: native CredentialStore and locked refresh.
// Copyright (c) 2025 Mario Zechner. MIT notice: ../UPSTREAM-LICENSE.
import { randomUUID } from 'node:crypto';
import { lstat, mkdir, open, readFile, rename, rm } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import type { AuthOperationOptions, Credential } from '@earendil-works/pi-ai';
import { CredentialLock } from './credential-lock.ts';
import { decodeCredentials, encodeCredentials } from './credential-codec.ts';

const hasCode = (error: unknown, code: string): boolean =>
  error instanceof Error && 'code' in error && error.code === code;
export interface Mutation<T> {
  readonly result: T;
  readonly changed: boolean;
}

// Owns the private file and its locked, abort-aware read-modify-write boundary.
export class CredentialFile {
  readonly #path: string;

  public constructor(path: string) {
    this.#path = resolve(path);
  }

  public async read(): Promise<Map<string, Credential>> {
    try {
      const stat = await lstat(this.#path);
      if (!stat.isFile() || stat.size > 262144) {
        throw new Error('Credential file must be a regular file of at most 256 KiB');
      }
      return decodeCredentials(await readFile(this.#path, 'utf8'));
    } catch (error) {
      if (hasCode(error, 'ENOENT')) {
        return new Map();
      }
      throw error;
    }
  }

  // Keep cancellation synchronous at the file-creation seam, before the transaction awaits.
  #ensureFile(signal?: AbortSignal): Promise<void> {
    signal?.throwIfAborted();
    return this.#createFile();
  }

  async #createFile(): Promise<void> {
    await mkdir(dirname(this.#path), { recursive: true, mode: 0o700 });
    try {
      const file = await open(this.#path, 'wx', 0o600);
      try {
        await file.writeFile('{}\n');
      } finally {
        await file.close();
      }
    } catch (error) {
      if (!hasCode(error, 'EEXIST')) {
        throw error;
      }
    }
    if (!(await lstat(this.#path)).isFile()) {
      throw new Error('Credential file must be regular, not a symlink');
    }
  }

  public async transaction<T>(
    apply: (data: Map<string, Credential>) => Promise<Mutation<T>>,
    options?: AuthOperationOptions,
  ): Promise<T> {
    await this.#ensureFile(options?.signal);
    const lock = await new CredentialLock(this.#path, options?.signal).acquire();
    try {
      lock.signal.throwIfAborted();
      const data = await this.read();
      const mutation = await apply(data);
      lock.signal.throwIfAborted();
      if (mutation.changed) {
        await this.#write(data, lock.signal);
      }
      return mutation.result;
    } finally {
      if (!lock.compromised) {
        await lock.release();
      }
    }
  }

  async #write(data: Map<string, Credential>, signal: AbortSignal): Promise<void> {
    const content = encodeCredentials(data);
    const temporary = this.#path + '.' + randomUUID() + '.tmp';
    try {
      const file = await open(temporary, 'wx', 0o600);
      try {
        await file.writeFile(content);
        await file.sync();
      } finally {
        await file.close();
      }
      signal.throwIfAborted();
      await rename(temporary, this.#path);
    } finally {
      await rm(temporary, { force: true });
    }
  }
}
