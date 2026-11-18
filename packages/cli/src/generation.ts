import { join } from 'node:path';
import { Configuration } from '@zf/config';
import type { Settings, ResolvedConfiguration } from '@zf/config';
import type { Loader, MachineResource, MachineSource } from '@zf/core';
import { ModuleCatalog, ToolCatalog } from '@zf/capabilities';
import type { MachineRoot, TypeScriptModuleLoader, OpenAIRuntime } from '@zf/capabilities';
import type { AssistantMessageEvent, ToolResultMessage } from '@earendil-works/pi-ai';
import { BUILTIN_MACHINES_DIRECTORY } from '@zf/machines';
import { builtinToolNames } from '@zf/tools/names';
import { GenerationBuilder } from './generation-builder.ts';
import { ToolPool } from './tool-pool.ts';
export interface GenerationOptions {
  readonly cwd: string;
  readonly globalDirectory: string;
  readonly runtime: OpenAIRuntime;
  readonly loader: TypeScriptModuleLoader;
  readonly overrides?: Readonly<Settings>;
  readonly onModelEvent?: (event: AssistantMessageEvent) => void;
  readonly onToolUpdate?: (result: ToolResultMessage) => void;
}
/** Candidate-local services: a refused reload cannot mutate the previous factory's bindings. */
export class ResourceLoader implements Loader {
  readonly #options: GenerationOptions;
  readonly #roots: readonly MachineRoot[];
  readonly #resourceRoots: readonly string[];
  public constructor(options: GenerationOptions) {
    this.#options = options;
    this.#resourceRoots = [options.globalDirectory, join(options.cwd, '.zedflow')];
    this.#roots = [
      { directory: BUILTIN_MACHINES_DIRECTORY, scope: 'builtin' },
      { directory: join(options.globalDirectory, 'machines'), scope: 'global' },
      { directory: join(options.cwd, '.zedflow', 'machines'), scope: 'project' },
    ];
  }
  public async list(): Promise<Map<string, MachineResource>> {
    return new ModuleCatalog({ roots: this.#roots, loader: this.#options.loader }).list();
  }
  public async configuration(id: string): Promise<ResolvedConfiguration> {
    const resource = (await this.list()).get(id);
    if (!resource) {
      throw new Error(`Unknown machine: ${id}`);
    }
    const discovered = await new ToolCatalog({ roots: this.#resourceRoots }).list();
    if (discovered.has('codemode')) {
      throw new Error('codemode is reserved for the native Code Mode capability');
    }
    return this.#resolve(resource, [...new Set([...builtinToolNames(), ...discovered.keys()])]);
  }
  public async load(path: string): Promise<MachineSource> {
    if (!path.startsWith('machine:')) {
      throw new Error('CLI runs require a discovered machine ID');
    }
    const id = path.slice(8);
    const resource = (await this.list()).get(id);
    if (!resource) {
      throw new Error(`Unknown machine: ${id}`);
    }
    const pool = new ToolPool(this.#options.cwd, this.#resourceRoots);
    await pool.load();
    const resolved = await this.#resolve(resource, pool.ids());
    return new GenerationBuilder(this.#options, resource, resolved, pool).load(path, this.#roots);
  }
  async #resolve(
    resource: MachineResource,
    toolIds: readonly string[],
  ): Promise<ResolvedConfiguration> {
    const configuration = new Configuration({
      globalDirectory: this.#options.globalDirectory,
      projectDirectory: this.#options.cwd,
      programDefaults: { tools: [...toolIds] },
    });
    return configuration.resolve({
      machineId: resource.id,
      defaults: {
        ...(resource.manifest.tools ? { tools: [...resource.manifest.tools] } : {}),
        ...resource.manifest.defaults,
      },
      ...(this.#options.overrides ? { overrides: this.#options.overrides } : {}),
    });
  }
}
