import { lstat, readdir } from 'node:fs/promises';
import type { Dirent } from 'node:fs';
import { basename, join } from 'node:path';
import { exists } from './files.ts';
import { machineId } from './identifiers.ts';
import { MachineManifestFile } from './machine-manifest.ts';
import type { DiscoveredMachine } from './types.ts';

const groupsMachines = (child: Dirent): boolean =>
  child.isDirectory() && !child.name.startsWith('.') && child.name !== 'node_modules';

/** One walk of a source tree; it collects the machines it finds, so each walk needs a new instance. */
export class MachineDiscovery {
  readonly #source: string;
  readonly #machines: DiscoveredMachine[] = [];

  public constructor(source: string) {
    this.#source = source;
  }

  public async discover(): Promise<readonly DiscoveredMachine[]> {
    if (await exists(join(this.#source, 'machine.json'))) {
      const manifest = await new MachineManifestFile(this.#source).read();
      return [{ id: manifest.name, relativeDirectory: '', manifest }];
    }
    const root = join(this.#source, 'machines');
    if (!(await exists(root))) {
      throw new Error('Source has neither machine.json nor machines/<id>/machine.json');
    }
    await this.#visit(root, '');
    if (this.#machines.length === 0) {
      throw new Error('Source contains no machines');
    }
    return this.#machines.sort((left, right) => left.id.localeCompare(right.id));
  }

  async #visit(directory: string, id: string): Promise<void> {
    if (!(await lstat(directory)).isDirectory()) {
      throw new Error(`Machine directories must not be symlinks: ${directory}`);
    }
    if (await exists(join(directory, 'machine.json'))) {
      await this.#addMachine(directory, id);
    } else {
      await this.#visitChildren(directory, id);
    }
  }

  async #addMachine(directory: string, id: string): Promise<void> {
    if (!machineId(id)) {
      throw new Error(`Invalid machine ID: ${id}`);
    }
    const manifest = await new MachineManifestFile(directory).read();
    if (manifest.name !== basename(directory)) {
      throw new Error(`Machine name must match its directory: ${id}`);
    }
    this.#machines.push({ id, relativeDirectory: `machines/${id}`, manifest });
  }

  async #visitChildren(directory: string, id: string): Promise<void> {
    for (const child of await readdir(directory, { withFileTypes: true })) {
      if (child.isSymbolicLink()) {
        throw new Error(`Symlink in machine discovery: ${child.name}`);
      }
      if (groupsMachines(child)) {
        await this.#visit(join(directory, child.name), id ? `${id}/${child.name}` : child.name);
      }
    }
  }
}

export const discoverMachines = async (source: string): Promise<readonly DiscoveredMachine[]> =>
  new MachineDiscovery(source).discover();
