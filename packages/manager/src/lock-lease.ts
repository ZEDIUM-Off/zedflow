import { lstat, mkdir } from 'node:fs/promises';
import type { Stats } from 'node:fs';
import { join, parse } from 'node:path';
import lockfile from 'proper-lockfile';
import { safePath } from './files.ts';
import type { MachineLayout } from './machine-layout.ts';

/** One acquired filesystem lock, including its original identity and compromise notification. */
export class LockLease {
  readonly #layout: MachineLayout;
  #identity!: Stats;
  #release!: () => Promise<void>;
  #compromised: Error | undefined;

  private constructor(layout: MachineLayout) {
    this.#layout = layout;
  }

  public static async acquire(layout: MachineLayout): Promise<LockLease> {
    const lease = new LockLease(layout);
    await lease.#initialize();
    await lease.#acquire();
    return lease;
  }

  public get compromised(): Error | undefined {
    return this.#compromised;
  }

  public async release(): Promise<void> {
    await this.#release();
  }

  public async assertOwned(): Promise<void> {
    try {
      const current = await lstat(join(this.#layout.metadata, 'operation.lock'));
      if (
        !current.isDirectory() ||
        current.ino !== this.#identity.ino ||
        current.dev !== this.#identity.dev
      ) {
        this.#compromised ??= new Error('Manager lock directory was replaced');
      }
    } catch (error) {
      this.#compromised ??= new Error('Manager lock directory is unavailable', { cause: error });
    }
    if (this.#compromised) {
      throw new Error('Manager lock ownership was lost', { cause: this.#compromised });
    }
  }

  async #initialize(): Promise<void> {
    await safePath(parse(this.#layout.directory).root, this.#layout.directory);
    for (const child of ['', 'snapshots', 'staging', 'runtime']) {
      const path = join(this.#layout.metadata, child);
      await safePath(this.#layout.directory, path);
      await mkdir(path, { recursive: true });
    }
  }

  async #acquire(): Promise<void> {
    this.#release = await lockfile.lock(this.#layout.directory, {
      lockfilePath: join(this.#layout.metadata, 'operation.lock'),
      realpath: false,
      retries: { retries: 40, minTimeout: 50, maxTimeout: 50, factor: 1 },
      stale: 30000,
      update: 10000,
      onCompromised: (error) => {
        this.#compromised = error;
      },
    });
    this.#identity = await lstat(join(this.#layout.metadata, 'operation.lock'));
  }
}
