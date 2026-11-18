import { join } from 'node:path';
import { exists } from './files.ts';
import { InstallVerifier } from './install-verifier.ts';
import type { MachineLayout } from './machine-layout.ts';
import type { InstallerProcess } from './process.ts';
import { SourcePreparation } from './source-preparation.ts';
import type { LockedSource, PreparedSource } from './types.ts';
import { discoverMachines } from './validation.ts';

/** Tracks only reconstructed snapshots, so verified existing snapshots are never promoted again. */
export class SnapshotRecovery {
  readonly #layout: MachineLayout;
  readonly #preparation: SourcePreparation;
  readonly #prepared: PreparedSource[] = [];

  public constructor(layout: MachineLayout, process: InstallerProcess, staging: string) {
    this.#layout = layout;
    this.#preparation = new SourcePreparation(layout.directory, process, staging);
  }

  public get prepared(): readonly PreparedSource[] {
    return this.#prepared;
  }

  public async snapshot(id: string, source: LockedSource): Promise<PreparedSource> {
    if (await exists(this.#layout.snapshot(id))) {
      await new InstallVerifier(this.#layout).snapshot(id, source);
      return {
        installId: id,
        directory: this.#layout.snapshot(id),
        locked: source,
        machines: await discoverMachines(join(this.#layout.snapshot(id), 'source')),
      };
    }
    const snapshot = await this.#preparation.prepare(source.source, { id, locked: source });
    this.#prepared.push(snapshot);
    return snapshot;
  }
}
