import { InstallVerifier } from './install-verifier.ts';
import type { MachineLayout } from './machine-layout.ts';
import type { OperationSession } from './operation-session.ts';
import { assertNoIdentityConflict, lockedSource } from './ownership.ts';
import type { InstallerProcess } from './process.ts';
import { SourcePreparation } from './source-preparation.ts';
import { SourceStaging } from './source-staging.ts';
import { TransactionCommit } from './transaction-commit.ts';
import type {
  DiscoveredMachine,
  InstallReceipt,
  LockedSource,
  ManagerLock,
  PreparedSource,
} from './types.ts';

/** Owns the currently installed sibling group; update may replace that group but never re-add removed siblings. */
export class SourceUpdate {
  readonly #layout: MachineLayout;
  readonly #transaction: OperationSession;
  readonly #current: ManagerLock;
  readonly #receipt: InstallReceipt;
  readonly #locked: LockedSource;
  readonly #group: readonly InstallReceipt[];
  readonly #remaining: readonly InstallReceipt[];

  public constructor(
    layout: MachineLayout,
    transaction: OperationSession,
    current: ManagerLock,
    receipt: InstallReceipt,
  ) {
    this.#layout = layout;
    this.#transaction = transaction;
    this.#current = current;
    this.#receipt = receipt;
    this.#locked = lockedSource(current, receipt.installId);
    this.#group = current.receipts.filter((item) => item.installId === receipt.installId);
    this.#remaining = current.receipts.filter((item) => item.installId !== receipt.installId);
  }

  public async update(process: InstallerProcess): Promise<readonly InstallReceipt[]> {
    await this.#verify();
    const prepared = await new SourcePreparation(
      this.#layout.directory,
      process,
      this.#transaction.directory,
    ).prepare(this.#locked.source);
    const retained = this.#retained(prepared);
    const created = await new SourceStaging(this.#layout, this.#transaction.directory, {
      ...prepared,
      machines: retained,
    }).prepare();
    await new TransactionCommit(this.#layout, this.#transaction, {
      current: this.#current,
      next: this.#next(prepared, created.receipts),
      prepared: [prepared],
      changes: created.changes,
    }).commit();
    return created.receipts;
  }

  async #verify(): Promise<void> {
    const verifier = new InstallVerifier(this.#layout);
    await verifier.snapshot(this.#receipt.installId, this.#locked);
    for (const item of this.#group) {
      await verifier.visible(item, this.#locked, false);
    }
  }

  #retained(prepared: PreparedSource): readonly DiscoveredMachine[] {
    const retained = prepared.machines.filter((machine) =>
      this.#group.some((item) => item.id === machine.id),
    );
    if (retained.length !== this.#group.length) {
      throw new Error(
        'Updated source removed/renamed an owned machine; remove it explicitly first',
      );
    }
    for (const machine of retained) {
      assertNoIdentityConflict(machine.id, this.#remaining);
    }
    return retained;
  }

  #next(prepared: PreparedSource, receipts: readonly InstallReceipt[]): ManagerLock {
    const sources = { ...this.#current.sources, [prepared.installId]: prepared.locked };
    delete sources[this.#receipt.installId];
    return { schemaVersion: 1, sources, receipts: [...this.#remaining, ...receipts] };
  }
}
