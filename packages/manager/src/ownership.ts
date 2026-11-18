import type { InstallReceipt, LockedSource, ManagerLock } from './types.ts';
import { machineId } from './validation.ts';

export const assertId = (id: string): void => {
  if (!machineId(id)) {
    throw new Error(`Invalid machine ID: ${id}`);
  }
};

export const owned = (lock: ManagerLock, id: string): InstallReceipt => {
  const receipt = lock.receipts.find((item) => item.id === id);
  if (!receipt) {
    throw new Error(`Machine is not manager-owned: ${id}`);
  }
  return receipt;
};

export const lockedSource = (lock: ManagerLock, id: string): LockedSource => {
  const source = lock.sources[id];
  if (!source) {
    throw new Error(`Missing locked source: ${id}`);
  }
  return source;
};

export const assertNoIdentityConflict = (id: string, receipts: readonly InstallReceipt[]): void => {
  if (
    receipts.some(
      (item) => item.id === id || item.id.startsWith(`${id}/`) || id.startsWith(`${item.id}/`),
    )
  ) {
    throw new Error(`Machine ownership conflict: ${id}`);
  }
};
