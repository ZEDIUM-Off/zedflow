import { integrity, machineId } from './identifiers.ts';
import { object } from './json-values.ts';
import { LockedSourceRecord } from './locked-source.ts';
import type { ManagerLock } from './types.ts';

type IdentifiedReceipt = Record<string, unknown> & {
  readonly id: string;
  readonly installId: string;
};
const hasIdentity = (receipt: unknown): receipt is IdentifiedReceipt =>
  object(receipt) &&
  typeof receipt['id'] === 'string' &&
  machineId(receipt['id']) &&
  typeof receipt['installId'] === 'string';

/**
 * Validates one raw manager lock. It owns the raw value while it checks every source record and
 * then every receipt against them, and tracks which machine IDs and sources the receipts claim.
 */
export class LockDecoder {
  readonly #lock: Record<string, unknown>;
  readonly #rawSources: Record<string, unknown>;
  readonly #receipts: readonly unknown[];
  readonly #sources = new Map<string, LockedSourceRecord>();
  readonly #claimedIds = new Set<string>();
  readonly #referencedSources = new Set<string>();

  public constructor(value: unknown) {
    if (
      !object(value) ||
      value['schemaVersion'] !== 1 ||
      !object(value['sources']) ||
      !Array.isArray(value['receipts'])
    ) {
      throw new Error('Invalid manager lock');
    }
    this.#lock = value;
    this.#rawSources = value['sources'];
    this.#receipts = value['receipts'];
  }

  public decode(): ManagerLock {
    this.#checkGroupDirectories();
    for (const [id, record] of Object.entries(this.#rawSources)) {
      this.#sources.set(id, new LockedSourceRecord(id, record));
    }
    for (const receipt of this.#receipts) {
      this.#claim(receipt);
    }
    if ([...this.#sources.keys()].some((id) => !this.#referencedSources.has(id))) {
      throw new Error('Unreferenced source in manager lock');
    }
    return this.#lock as unknown as ManagerLock;
  }

  #checkGroupDirectories(): void {
    const groups = this.#lock['groupDirectories'];
    if (
      groups !== undefined &&
      (!Array.isArray(groups) ||
        groups.some((group) => typeof group !== 'string' || !machineId(group)) ||
        new Set(groups).size !== groups.length)
    ) {
      throw new Error('Invalid manager-owned grouping directories');
    }
  }

  #claim(receipt: unknown): void {
    if (!hasIdentity(receipt)) {
      throw new Error('Invalid install receipt');
    }
    const id = receipt.id;
    const source = this.#sources.get(receipt.installId);
    if (
      source === undefined ||
      !source.justifies(receipt) ||
      !integrity(receipt['installedIntegrity']) ||
      receipt['destination'] !== `machines/${id}` ||
      this.#overlapsClaimedId(id)
    ) {
      throw new Error('Invalid ownership/integrity receipt');
    }
    this.#claimedIds.add(id);
    this.#referencedSources.add(receipt.installId);
  }

  #overlapsClaimedId(id: string): boolean {
    return (
      this.#claimedIds.has(id) ||
      [...this.#claimedIds].some(
        (claimed) => claimed.startsWith(`${id}/`) || id.startsWith(`${claimed}/`),
      )
    );
  }
}

export const validateLock = (value: unknown): ManagerLock => new LockDecoder(value).decode();
