import { createHash } from 'node:crypto';
import type {
  Loader,
  MachineBindings,
  MachineFactory,
  MachineResource,
  MachineSource,
} from '@zf/core';
import { MachineManifestDecoder } from './machine-manifest-decoder.ts';
import {
  confinedEntry,
  discoverDeclarations,
  readDeclaration,
  validResourceId,
} from './resource-files.ts';

export interface MachineRoot {
  readonly directory: string;
  readonly scope: MachineResource['scope'];
}
export interface ModuleCatalogOptions {
  readonly roots: readonly MachineRoot[];
  readonly loader: Loader;
  readonly bindings?: (resource: MachineResource) => Promise<MachineBindings>;
}
const priority = { builtin: 0, global: 1, project: 2 } as const;
export class ModuleCatalog implements Loader {
  readonly #roots: readonly MachineRoot[];
  readonly #loader: Loader;
  readonly #bindings: ModuleCatalogOptions['bindings'];

  public constructor(options: ModuleCatalogOptions) {
    this.#roots = options.roots
      .map((root) => ({ ...root }))
      .sort((a, b) => priority[a.scope] - priority[b.scope]);
    this.#loader = options.loader;
    this.#bindings = options.bindings;
  }
  public async list(): Promise<Map<string, MachineResource>> {
    const resources = new Map<string, MachineResource>();
    const seen = new Set<string>();
    for (const root of this.#roots) {
      for (const file of await discoverDeclarations(root.directory, 'machine.json')) {
        const key = `${root.scope}:${file.id}`;
        if (seen.has(key)) {
          throw new Error(`Duplicate ${root.scope} machine ID: ${file.id}`);
        }
        seen.add(key);
        try {
          resources.set(file.id, {
            id: file.id,
            directory: file.directory,
            scope: root.scope,
            manifest: await new MachineManifestDecoder(
              file.directory,
              await readDeclaration(file.path),
            ).decode(),
          });
        } catch (error) {
          throw new Error(
            `Invalid machine declaration ${file.path}: ${error instanceof Error ? error.message : 'Invalid value'}`,
          );
        }
      }
    }
    return resources;
  }
  public async resolve(name: string): Promise<string> {
    await this.#resource(name);
    return `machine:${name}`;
  }
  public async load(path: string): Promise<MachineSource> {
    if (!path.startsWith('machine:')) {
      return this.#loader.load(path);
    }
    const id = path.slice('machine:'.length);
    const resource = await this.#resource(id);
    const entry = await confinedEntry(resource.directory, resource.manifest.entry);
    const loaded = await this.#loader.load(entry, resource.directory);
    const { module } = await this.#instantiate(loaded.module, resource);
    const after = (await this.list()).get(id);
    if (JSON.stringify(resource) !== JSON.stringify(after)) {
      throw new Error('Machine declaration changed while loading');
    }
    const revision = createHash('sha256')
      .update(loaded.revision)
      .update(JSON.stringify(resource))
      .digest('hex');
    return { path, revision, module };
  }
  async #resource(name: string): Promise<MachineResource> {
    if (!validResourceId(name)) {
      throw new Error('Invalid machine name');
    }
    const resource = (await this.list()).get(name);
    if (!resource) {
      throw new Error(`Unknown machine: ${name}`);
    }
    return resource;
  }
  /**
   * Binds a machine factory module to native capabilities; other modules are returned as loaded.
   * The envelope keeps the raw module or factory result opaque: an async function would otherwise
   * adopt a returned Promise or thenable instead of leaving it for engine admission.
   */
  async #instantiate(
    module: unknown,
    resource: MachineResource,
  ): Promise<Pick<MachineSource, 'module'>> {
    if (
      !module ||
      typeof module !== 'object' ||
      !('createMachine' in module) ||
      typeof module.createMachine !== 'function'
    ) {
      return { module };
    }
    if (!this.#bindings) {
      throw new Error('Machine factory requires native capability bindings');
    }
    const factory = module.createMachine as MachineFactory;
    return { module: factory(await this.#bindings(resource)) };
  }
}
