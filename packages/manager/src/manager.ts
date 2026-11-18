import { FrozenRestore } from './frozen-restore.ts';
import { MachineLayout } from './machine-layout.ts';
import { MachineRemoval } from './machine-removal.ts';
import { ManagerLockStore } from './manager-lock-store.ts';
import { assertId, owned } from './ownership.ts';
import { InstallerProcess, NativeProcessRunner } from './process.ts';
import { SourceInstallation } from './source-installation.ts';
import { SourceUpdate } from './source-update.ts';
import type { InstallReceipt, MachineManagerOptions } from './types.ts';
import { parseSource } from './validation.ts';

/**
 * Installs metadata/source only; no machine entry or lifecycle module is imported.
 * Ordinary failures roll back, but multiple directory renames are not visibility-atomic or
 * power-loss/process-kill recoverable. Interrupted content/ownership mismatches refuse overwrite;
 * retained staging requires manual recovery. CommittedCleanupError means publication succeeded
 * and recovery evidence remains: inspect readable receipts rather than blindly retrying install.
 */
export class MachineManager {
  readonly #layout: MachineLayout;
  readonly #locks: ManagerLockStore;
  readonly #process: InstallerProcess;

  public constructor(options: MachineManagerOptions) {
    this.#layout = new MachineLayout(options.directory);
    this.#locks = new ManagerLockStore(this.#layout);
    this.#process = new InstallerProcess({
      runner: options.process ?? new NativeProcessRunner(),
      runtime: this.#layout.runtime,
      registry: options.registry ?? 'https://registry.npmjs.org/',
    });
  }

  public async list(): Promise<readonly InstallReceipt[]> {
    return (await this.#locks.read()).receipts;
  }

  public async install(source: string): Promise<readonly InstallReceipt[]> {
    const parsed = parseSource(source);
    return this.#locks.run(async (transaction) => {
      const current = await this.#locks.read();
      return new SourceInstallation(this.#layout, this.#process, transaction, current).install(
        parsed,
      );
    });
  }

  public async update(id: string): Promise<readonly InstallReceipt[]> {
    assertId(id);
    return this.#locks.run(async (transaction) => {
      const current = await this.#locks.read();
      return new SourceUpdate(this.#layout, transaction, current, owned(current, id)).update(
        this.#process,
      );
    });
  }

  public async remove(id: string): Promise<void> {
    assertId(id);
    await this.#locks.run(async (transaction) => {
      const current = await this.#locks.read();
      await new MachineRemoval(this.#layout, transaction, current, owned(current, id)).remove();
    });
  }

  public async restore(options: { readonly frozen: true }): Promise<readonly InstallReceipt[]> {
    if (options.frozen !== true) {
      throw new Error('Restore requires frozen:true');
    }
    return this.#locks.run(async (transaction) => {
      const current = await this.#locks.read();
      return new FrozenRestore(this.#layout, this.#process, transaction, current).restore();
    });
  }
}
