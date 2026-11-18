import { rename, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { atomicJson, exists, safePath } from './files.ts';
import type { MachineLayout } from './machine-layout.ts';
import type { OperationSession } from './operation-session.ts';
import type { GenerationChange } from './transaction-types.ts';
import { VisiblePublication } from './visible-publication.ts';

/** Owns snapshot promotion and rollback across one generation's visible publication and lock write. */
export class TransactionCommit {
  readonly #layout: MachineLayout;
  readonly #transaction: OperationSession;
  readonly #change: GenerationChange;
  readonly #visible: VisiblePublication;
  readonly #promoted: string[] = [];

  public constructor(
    layout: MachineLayout,
    transaction: OperationSession,
    change: GenerationChange,
  ) {
    this.#layout = layout;
    this.#transaction = transaction;
    this.#change = change;
    this.#visible = new VisiblePublication(layout, transaction, change.current);
  }

  public async commit(): Promise<void> {
    await this.#visible.preflight(this.#change.changes);
    await this.#transaction.assertLock();
    try {
      await this.#promote();
      for (const change of this.#change.changes) {
        await this.#visible.apply(change);
      }
      await this.#visible.pruneGroups();
      await this.#publish();
    } catch (error) {
      await this.#rollback(error);
      throw error;
    }
    await this.#removeObsoleteSnapshots();
  }

  async #publish(): Promise<void> {
    await this.#transaction.assertLock();
    await atomicJson(join(this.#layout.metadata, 'lock.json'), {
      ...this.#change.next,
      groupDirectories: this.#visible.groups,
    });
    this.#transaction.markCommitted();
  }

  async #promote(): Promise<void> {
    for (const prepared of this.#change.prepared) {
      const destination = this.#layout.snapshot(prepared.installId);
      await safePath(this.#layout.directory, destination);
      if (await exists(destination)) {
        throw new Error('Snapshot ownership conflict');
      }
      await rename(prepared.directory, destination);
      this.#promoted.push(destination);
    }
  }

  async #rollback(error: unknown): Promise<void> {
    try {
      await this.#visible.rollback();
      for (const path of this.#promoted) {
        await rm(path, { recursive: true, force: true });
      }
    } catch (rollbackError) {
      this.#transaction.preserve();
      throw new AggregateError(
        [error, rollbackError],
        `Rollback could not finish; recovery files retained at ${this.#transaction.directory}`,
      );
    }
  }

  async #removeObsoleteSnapshots(): Promise<void> {
    for (const id of Object.keys(this.#change.current.sources)) {
      if (this.#change.next.sources[id] === undefined) {
        await rm(this.#layout.snapshot(id), { recursive: true, force: true });
      }
    }
  }
}
