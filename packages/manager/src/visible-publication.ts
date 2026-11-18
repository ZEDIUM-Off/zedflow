import { mkdir, rename, rm } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { exists } from './files.ts';
import { InstallVerifier } from './install-verifier.ts';
import type { MachineLayout } from './machine-layout.ts';
import type { OperationSession } from './operation-session.ts';
import { lockedSource } from './ownership.ts';
import type { Change } from './transaction-types.ts';
import type { ManagerLock } from './types.ts';

interface MovedMachine {
  readonly target: string;
  readonly backup?: string;
  replacement: boolean;
}

/** Reversible publication of visible machines; owns the move and namespace-directory journals. */
export class VisiblePublication {
  readonly #layout: MachineLayout;
  readonly #transaction: OperationSession;
  readonly #current: ManagerLock;
  readonly #moved: MovedMachine[] = [];
  readonly #groups: Set<string>;
  readonly #createdGroups: string[] = [];
  readonly #removedGroups: string[] = [];

  public constructor(layout: MachineLayout, transaction: OperationSession, current: ManagerLock) {
    this.#layout = layout;
    this.#transaction = transaction;
    this.#current = current;
    this.#groups = new Set(current.groupDirectories ?? []);
  }

  public get groups(): readonly string[] {
    return [...this.#groups].sort();
  }

  /** Preflight the whole transaction before any destination is changed. */
  public async preflight(changes: readonly Change[]): Promise<void> {
    const verifier = new InstallVerifier(this.#layout);
    for (const change of changes) {
      const target = await this.#layout.target(change.id);
      const owned = this.#current.receipts.find((item) => item.id === change.id);
      if (owned) {
        await verifier.visible(owned, lockedSource(this.#current, owned.installId), true);
      } else if (await exists(target)) {
        throw new Error(`User-authored machine directory is protected: ${change.id}`);
      }
    }
  }

  public async apply(change: Change): Promise<void> {
    await this.#transaction.assertLock();
    const target = await this.#layout.target(change.id);
    const backup = await this.#backup(target, change.id);
    const record = { target, ...(backup === undefined ? {} : { backup }), replacement: false };
    this.#moved.push(record);
    if (change.replacement) {
      await this.#layout.ensureGroups(target, this.#createdGroups);
      for (const group of this.#createdGroups) {
        this.#groups.add(group);
      }
      await rename(change.replacement, target);
      record.replacement = true;
    }
  }

  async #backup(target: string, id: string): Promise<string | undefined> {
    if (!(await exists(target))) {
      return undefined;
    }
    const backup = join(this.#transaction.directory, 'old', id);
    await mkdir(dirname(backup), { recursive: true });
    await rename(target, backup);
    return backup;
  }

  public async pruneGroups(): Promise<void> {
    await this.#layout.pruneGroups(this.#groups, this.#removedGroups);
  }

  public async rollback(): Promise<void> {
    for (const group of this.#removedGroups.reverse()) {
      await mkdir(join(this.#layout.directory, 'machines', group), { recursive: true });
    }
    for (const record of this.#moved.reverse()) {
      if (record.replacement) {
        await rm(record.target, { recursive: true, force: true });
      }
      if (record.backup) {
        await rename(record.backup, record.target);
      }
    }
    await this.#layout.pruneGroups(new Set(this.#createdGroups), []);
  }
}
