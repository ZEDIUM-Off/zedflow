import { setTimeout } from 'node:timers/promises';
import lockfile from 'proper-lockfile';
import { hasErrorCode } from './guards.ts';

/** One advisory lock on a settings file; a compromised lock is never released. */
export class SettingsLock {
  readonly #path: string;
  #release: (() => Promise<void>) | undefined;
  #compromised: Error | undefined;

  public constructor(path: string) {
    this.#path = path;
  }

  public async acquire(): Promise<void> {
    const deadline = Date.now() + 2000;
    while (this.#release === undefined) {
      try {
        this.#release = await this.#attempt();
      } catch (error) {
        await this.#retryOrThrow(error, deadline);
      }
    }
  }

  public assertHeld(): void {
    if (this.#compromised) {
      throw new Error(`Settings lock was lost: ${this.#path}`);
    }
  }

  public async release(): Promise<void> {
    if (!this.#compromised) {
      await this.#release?.();
    }
  }

  async #attempt(): Promise<() => Promise<void>> {
    return await lockfile.lock(this.#path, {
      realpath: false,
      retries: 0,
      stale: 30000,
      update: 10000,
      onCompromised: (error) => {
        this.#compromised = error;
      },
    });
  }

  async #retryOrThrow(error: unknown, deadline: number): Promise<void> {
    if (!hasErrorCode(error, 'ELOCKED')) {
      throw error;
    }
    if (Date.now() >= deadline) {
      throw new Error(`Settings are locked by another writer: ${this.#path}`);
    }
    await setTimeout(10);
  }
}
