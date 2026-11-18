import { mkdir, symlink } from 'node:fs/promises';
import { join } from 'node:path';
import { atomicJson, copySource, digestTree } from './files.ts';
import type { MachineLayout } from './machine-layout.ts';
import type { Change } from './transaction-types.ts';
import type { DiscoveredMachine, InstallReceipt, PreparedSource } from './types.ts';

/** Owns the staged replacements and receipts for one source selection, never visible destinations. */
export class SourceStaging {
  readonly #layout: MachineLayout;
  readonly #directory: string;
  readonly #source: PreparedSource;
  readonly #receipts: InstallReceipt[] = [];
  readonly #changes: Change[] = [];
  public constructor(layout: MachineLayout, directory: string, source: PreparedSource) {
    this.#layout = layout;
    this.#directory = directory;
    this.#source = source;
  }
  public async prepare(): Promise<{
    receipts: readonly InstallReceipt[];
    changes: readonly Change[];
  }> {
    for (const machine of this.#source.machines) {
      await this.#stage(machine);
    }
    return { receipts: this.#receipts, changes: this.#changes };
  }
  async #stage(machine: DiscoveredMachine): Promise<void> {
    const source = this.#source;
    const directory = join(this.#directory, 'new', machine.id);
    await mkdir(directory, { recursive: true });
    await copySource({ from: join(source.directory, 'source'), to: join(directory, '.source') });
    for (const [name, target] of Object.entries(
      this.#layout.links(source.installId, source.locked, machine.id),
    )) {
      await symlink(target, join(directory, name), 'dir');
    }
    await this.#manifest(machine, directory);
    this.#receipts.push(await this.#receipt(machine, directory));
    this.#changes.push({ id: machine.id, replacement: directory });
  }
  async #manifest(machine: DiscoveredMachine, directory: string): Promise<void> {
    const prefix = machine.relativeDirectory ? `.source/${machine.relativeDirectory}/` : '.source/';
    await atomicJson(join(directory, 'machine.json'), {
      ...machine.manifest,
      entry: `${prefix}${machine.manifest.entry}`,
      ...(machine.manifest.ui === undefined ? {} : { ui: `${prefix}${machine.manifest.ui}` }),
    });
  }
  async #receipt(machine: DiscoveredMachine, directory: string): Promise<InstallReceipt> {
    const source = this.#source;
    return {
      id: machine.id,
      source: source.locked.source.normalized,
      resolved: source.locked.resolved,
      installId: source.installId,
      destination: `machines/${machine.id}`,
      sourceIntegrity: source.locked.sourceIntegrity,
      dependencyIntegrity: source.locked.dependencyIntegrity,
      installedIntegrity: await digestTree({
        root: directory,
        links: this.#layout.links(source.installId, source.locked, machine.id),
      }),
    };
  }
}
