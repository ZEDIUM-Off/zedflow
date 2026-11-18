// Adapted from Pi 1.5.2 core/auth-storage.ts: locked credential refresh.
// Copyright (c) 2025 Mario Zechner. MIT notice: ../UPSTREAM-LICENSE.
import { setTimeout } from 'node:timers/promises';
import lockfile from 'proper-lockfile';

const isLocked = (error: unknown): boolean =>
  error instanceof Error && 'code' in error && error.code === 'ELOCKED';

// A lock lease owns compromise cancellation and must not release an already-lost lock.
export class CredentialLock {
  readonly #path: string;
  readonly #compromised = new AbortController();
  public readonly signal: AbortSignal;
  #release: (() => Promise<void>) | undefined;

  public constructor(path: string, signal?: AbortSignal) {
    this.#path = path;
    this.signal = signal
      ? AbortSignal.any([signal, this.#compromised.signal])
      : this.#compromised.signal;
  }

  public get compromised(): boolean {
    return this.#compromised.signal.aborted;
  }

  public release(): Promise<void> {
    if (!this.#release) {
      throw new Error('Credential lock not acquired');
    }
    return this.#release();
  }

  public async acquire(): Promise<this> {
    const deadline = Date.now() + 30000;
    while (true) {
      this.signal.throwIfAborted();
      try {
        this.#release = await lockfile.lock(this.#path, {
          retries: 0,
          stale: 30000,
          update: 10000,
          onCompromised: (error) => this.#compromised.abort(error),
        });
        return this;
      } catch (error) {
        if (!isLocked(error) || Date.now() >= deadline) {
          throw error;
        }
        await setTimeout(25, undefined, { signal: this.signal });
      }
    }
  }
}
