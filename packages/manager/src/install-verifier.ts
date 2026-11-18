import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { digestTree, exists, safePath } from './files.ts';
import type { MachineLayout } from './machine-layout.ts';
import type { InstallReceipt, LockedSource } from './types.ts';
import { readJson } from './validation.ts';

/** Verifies content against the immutable snapshot and installed-link policy of one layout. */
export class InstallVerifier {
  readonly #layout: MachineLayout;
  public constructor(layout: MachineLayout) {
    this.#layout = layout;
  }
  public async snapshot(id: string, source: LockedSource): Promise<void> {
    const snapshot = this.#layout.snapshot(id);
    await safePath(this.#layout.directory, snapshot);
    if ((await digestTree({ root: join(snapshot, 'source') })) !== source.sourceIntegrity) {
      throw new Error('Managed source snapshot was locally modified');
    }
    if (source.dependencyPath !== null) {
      await safePath(snapshot, join(snapshot, 'dependencies', 'node_modules'));
      if (
        JSON.stringify(await readJson(join(snapshot, 'dependencies', 'package.json'))) !==
          JSON.stringify(source.dependencyManifest) ||
        (await readFile(join(snapshot, 'dependencies', 'pnpm-lock.yaml'), 'utf8')) !==
          source.dependencyLock ||
        (await digestTree({
          root: join(snapshot, 'dependencies', 'node_modules'),
          links: 'dependencies',
        })) !== source.dependencyIntegrity
      ) {
        throw new Error('Managed dependency snapshot was locally modified');
      }
    }
  }

  public async visible(
    receipt: InstallReceipt,
    source: LockedSource,
    allowMissing: boolean,
  ): Promise<boolean> {
    const path = join(this.#layout.directory, receipt.destination);
    await this.#layout.target(receipt.id);
    if (!(await exists(path))) {
      if (allowMissing) {
        return false;
      }
      throw new Error(`Owned machine is missing; restore it first: ${receipt.id}`);
    }
    if (
      (await digestTree({
        root: path,
        links: this.#layout.links(receipt.installId, source, receipt.id),
      })) !== receipt.installedIntegrity
    ) {
      throw new Error(`Locally modified machine is protected: ${receipt.id}`);
    }
    return true;
  }
}
