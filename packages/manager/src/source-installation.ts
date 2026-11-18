import type { MachineLayout } from './machine-layout.ts';
import type { OperationSession } from './operation-session.ts';
import { assertNoIdentityConflict } from './ownership.ts';
import type { InstallerProcess } from './process.ts';
import { SourcePreparation } from './source-preparation.ts';
import { SourceStaging } from './source-staging.ts';
import { TransactionCommit } from './transaction-commit.ts';
import type { InstallReceipt, LockedSource, ManagerLock, Source } from './types.ts';

/** Owns the draft ownership generation of one install until it is published. */
export class SourceInstallation {
  readonly #layout: MachineLayout;
  readonly #process: InstallerProcess;
  readonly #transaction: OperationSession;
  readonly #current: ManagerLock;
  readonly #sources: Record<string, LockedSource>;
  readonly #receipts: InstallReceipt[];

  public constructor(
    layout: MachineLayout,
    process: InstallerProcess,
    transaction: OperationSession,
    current: ManagerLock,
  ) {
    this.#layout = layout;
    this.#process = process;
    this.#transaction = transaction;
    this.#current = current;
    this.#sources = { ...current.sources };
    this.#receipts = [...current.receipts];
  }

  public async install(source: Source): Promise<readonly InstallReceipt[]> {
    const prepared = await new SourcePreparation(
      this.#layout.directory,
      this.#process,
      this.#transaction.directory,
    ).prepare(source);
    for (const machine of prepared.machines) {
      assertNoIdentityConflict(machine.id, this.#receipts);
    }
    const created = await new SourceStaging(
      this.#layout,
      this.#transaction.directory,
      prepared,
    ).prepare();
    this.#sources[prepared.installId] = prepared.locked;
    this.#receipts.push(...created.receipts);
    await new TransactionCommit(this.#layout, this.#transaction, {
      current: this.#current,
      next: { schemaVersion: 1, sources: this.#sources, receipts: this.#receipts },
      prepared: [prepared],
      changes: created.changes,
    }).commit();
    return created.receipts;
  }
}
