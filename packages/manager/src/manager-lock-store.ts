import { randomUUID } from 'node:crypto';
import { lstat } from 'node:fs/promises';
import { join, parse } from 'node:path';
import { exists, safePath } from './files.ts';
import { LockLease } from './lock-lease.ts';
import type { MachineLayout } from './machine-layout.ts';
import { OperationSession } from './operation-session.ts';
import type { ManagerLock } from './types.ts';
import { readJson, validateLock } from './validation.ts';

/** Owns access to one manager's lock document and serializes operations on it. Reads create no state. */
export class ManagerLockStore {
  readonly #layout: MachineLayout;
  public constructor(layout: MachineLayout) {
    this.#layout = layout;
  }

  public async read(): Promise<ManagerLock> {
    await safePath(parse(this.#layout.directory).root, this.#layout.directory);
    const path = join(this.#layout.metadata, 'lock.json');
    await safePath(this.#layout.directory, path);
    if (!(await exists(path))) {
      return { schemaVersion: 1, sources: {}, receipts: [] };
    }
    if (!(await lstat(path)).isFile()) {
      throw new Error('Manager lock must be a regular file');
    }
    return validateLock(await readJson(path));
  }

  public async run<T>(operation: (transaction: OperationSession) => Promise<T>): Promise<T> {
    const lease = await LockLease.acquire(this.#layout);
    const directory = join(this.#layout.metadata, 'staging', randomUUID());
    return new OperationSession(directory, lease).run(operation);
  }
}
