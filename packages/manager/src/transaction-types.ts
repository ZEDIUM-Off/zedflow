import type { ManagerLock, PreparedSource } from './types.ts';

export interface Change {
  readonly id: string;
  readonly replacement?: string;
}
export interface GenerationChange {
  readonly current: ManagerLock;
  readonly next: ManagerLock;
  readonly prepared: readonly PreparedSource[];
  readonly changes: readonly Change[];
}
