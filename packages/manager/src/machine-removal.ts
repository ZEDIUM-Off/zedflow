import { InstallVerifier } from './install-verifier.ts';
import type { MachineLayout } from './machine-layout.ts';
import type { OperationSession } from './operation-session.ts';
import { lockedSource } from './ownership.ts';
import { TransactionCommit } from './transaction-commit.ts';
import type { InstallReceipt, LockedSource, ManagerLock } from './types.ts';

/** One owned machine's removal, retaining its source snapshot while any sibling still owns it. */
export class MachineRemoval {
  readonly #layout: MachineLayout;
  readonly #transaction: OperationSession;
  readonly #current: ManagerLock;
  readonly #receipt: InstallReceipt;
  readonly #source: LockedSource;

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
    this.#source = lockedSource(current, receipt.installId);
  }

  public async remove(): Promise<void> {
    const verifier = new InstallVerifier(this.#layout);
    await verifier.visible(this.#receipt, this.#source, false);
    await verifier.snapshot(this.#receipt.installId, this.#source);
    await new TransactionCommit(this.#layout, this.#transaction, {
      current: this.#current,
      next: this.#next(),
      prepared: [],
      changes: [{ id: this.#receipt.id }],
    }).commit();
  }

  #next(): ManagerLock {
    const receipts = this.#current.receipts.filter((item) => item.id !== this.#receipt.id);
    const sources = { ...this.#current.sources };
    if (!receipts.some((item) => item.installId === this.#receipt.installId)) {
      delete sources[this.#receipt.installId];
    }
    return { schemaVersion: 1, sources, receipts };
  }
}
