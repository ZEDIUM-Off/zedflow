import { mkdir, rm } from 'node:fs/promises';
import { CommittedCleanupError } from './errors.ts';
import type { LockLease } from './lock-lease.ts';

type Outcome<T> = { value: T } | { error: unknown };

/** Owns one staging lifetime and its publication/recovery status; releases before removing staging. */
export class OperationSession {
  readonly #directory: string;
  readonly #lease: LockLease;
  readonly #cleanupErrors: unknown[] = [];
  #preserve = false;
  #committed = false;
  #stagingRemoved = false;

  public constructor(directory: string, lease: LockLease) {
    this.#directory = directory;
    this.#lease = lease;
  }

  public get directory(): string {
    return this.#directory;
  }
  public async assertLock(): Promise<void> {
    await this.#lease.assertOwned();
  }
  public preserve(): void {
    this.#preserve = true;
  }
  public markCommitted(): void {
    this.#committed = true;
  }

  public async run<T>(operation: (transaction: OperationSession) => Promise<T>): Promise<T> {
    let outcome: Outcome<T>;
    try {
      await mkdir(this.#directory);
      outcome = { value: await operation(this) };
    } catch (error) {
      outcome = { error };
    }
    await this.#cleanup('error' in outcome);
    return this.#finish(outcome);
  }

  async #cleanup(failed: boolean): Promise<void> {
    if (!this.#lease.compromised) {
      try {
        await this.#lease.release();
      } catch (error) {
        this.#cleanupErrors.push(error);
      }
    } else if (this.#committed) {
      this.#cleanupErrors.push(this.#lease.compromised);
    }
    if (this.#committed && (failed || this.#cleanupErrors.length > 0)) {
      this.#preserve = true;
    }
    await this.#removeStaging();
  }

  async #removeStaging(): Promise<void> {
    if (!this.#preserve) {
      try {
        await rm(this.#directory, { recursive: true, force: true });
        this.#stagingRemoved = true;
      } catch (error) {
        this.#cleanupErrors.push(error);
      }
    }
  }

  #finish<T>(outcome: Outcome<T>): T {
    const errors = [...('error' in outcome ? [outcome.error] : []), ...this.#cleanupErrors];
    if (errors.length > 0) {
      this.#throwErrors(errors);
    }
    if ('error' in outcome) {
      throw outcome.error;
    }
    return outcome.value;
  }

  #throwErrors(errors: unknown[]): never {
    if (this.#committed) {
      throw new CommittedCleanupError({
        recoveryDirectory: this.#directory,
        cause: new AggregateError(errors),
      });
    }
    if (errors.length > 1) {
      throw new AggregateError(
        errors,
        this.#preserve
          ? `Manager operation and cleanup failed; recovery location: ${this.#directory}`
          : this.#stagingRemoved
            ? 'Manager operation and cleanup failed; staging was removed'
            : 'Manager operation and cleanup failed; staging cleanup is incomplete',
      );
    }
    throw errors[0];
  }
}
