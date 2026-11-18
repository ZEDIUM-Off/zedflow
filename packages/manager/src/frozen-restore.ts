import { InstallVerifier } from './install-verifier.ts';
import type { MachineLayout } from './machine-layout.ts';
import type { OperationSession } from './operation-session.ts';
import type { InstallerProcess } from './process.ts';
import { SnapshotRecovery } from './snapshot-recovery.ts';
import { SourceStaging } from './source-staging.ts';
import { TransactionCommit } from './transaction-commit.ts';
import type { Change } from './transaction-types.ts';
import type { InstallReceipt, LockedSource, ManagerLock, PreparedSource } from './types.ts';

/** Owns the complete frozen recovery generation; none of its missing machines is published early. */
export class FrozenRestore {
  readonly #layout: MachineLayout;
  readonly #transaction: OperationSession;
  readonly #current: ManagerLock;
  readonly #snapshots: SnapshotRecovery;
  readonly #verifier: InstallVerifier;
  readonly #changes: Change[] = [];

  public constructor(
    layout: MachineLayout,
    process: InstallerProcess,
    transaction: OperationSession,
    current: ManagerLock,
  ) {
    this.#layout = layout;
    this.#transaction = transaction;
    this.#current = current;
    this.#snapshots = new SnapshotRecovery(layout, process, transaction.directory);
    this.#verifier = new InstallVerifier(layout);
  }

  public async restore(): Promise<readonly InstallReceipt[]> {
    for (const [id, source] of Object.entries(this.#current.sources)) {
      const snapshot = await this.#snapshots.snapshot(id, source);
      await this.#restoreReceipts(snapshot, id, source);
    }
    await new TransactionCommit(this.#layout, this.#transaction, {
      current: this.#current,
      next: this.#current,
      prepared: this.#snapshots.prepared,
      changes: this.#changes,
    }).commit();
    return this.#current.receipts;
  }

  async #restoreReceipts(
    snapshot: PreparedSource,
    id: string,
    source: LockedSource,
  ): Promise<void> {
    for (const receipt of this.#current.receipts.filter((item) => item.installId === id)) {
      const present = await this.#verifier.visible(receipt, source, true);
      if (!present) {
        await this.#stageMissing(snapshot, receipt);
      }
    }
  }

  async #stageMissing(snapshot: PreparedSource, receipt: InstallReceipt): Promise<void> {
    const machine = snapshot.machines.find((item) => item.id === receipt.id);
    if (!machine) {
      throw new Error(`Frozen source is missing ${receipt.id}`);
    }
    const staged = await new SourceStaging(this.#layout, this.#transaction.directory, {
      ...snapshot,
      machines: [machine],
    }).prepare();
    if (staged.receipts[0]?.installedIntegrity !== receipt.installedIntegrity) {
      throw new Error(`Frozen installed content drift: ${receipt.id}`);
    }
    this.#changes.push(...staged.changes);
  }
}
