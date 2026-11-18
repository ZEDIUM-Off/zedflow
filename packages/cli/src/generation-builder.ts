import { createHash } from 'node:crypto';
import { join, resolve } from 'node:path';
import { compile } from '@earendil-works/pi-schema';
import type { ResolvedConfiguration, Settings } from '@zf/config';
import type {
  ExecutableTool,
  MachineBindings,
  MachineResource,
  MachineSource,
  Skill,
} from '@zf/core';
import { ModuleCatalog, SkillCatalog } from '@zf/capabilities';
import type { MachineRoot } from '@zf/capabilities';
import type { GenerationOptions } from './generation.ts';
import { freezeSettings } from './generation-settings.ts';
import type { ToolPool } from './tool-pool.ts';

/** Owns one immutable candidate's configuration and bound capabilities, never a live generation. */
export class GenerationBuilder {
  readonly #options: GenerationOptions;
  readonly #resource: MachineResource;
  readonly #resolved: ResolvedConfiguration;
  readonly #pool: ToolPool;
  readonly #settings: Readonly<Settings>;
  public constructor(
    options: GenerationOptions,
    resource: MachineResource,
    resolved: ResolvedConfiguration,
    pool: ToolPool,
  ) {
    this.#options = options;
    this.#resource = resource;
    this.#resolved = resolved;
    this.#pool = pool;
    this.#settings = freezeSettings(resolved.settings);
    if (
      resource.manifest.configSchema &&
      !compile(resource.manifest.configSchema).check(this.#settings)
    ) {
      throw new Error(`Configuration does not match the schema for machine ${resource.id}`);
    }
  }
  public async load(path: string, roots: readonly MachineRoot[]): Promise<MachineSource> {
    const tools = await this.#tools();
    const skills = new SkillCatalog([
      this.#options.globalDirectory,
      join(this.#options.cwd, '.zedflow'),
    ]);
    const skillMetadata = [...(await skills.list()).values()];
    const skillContents = await Promise.all(skillMetadata.map((skill) => skills.read(skill.name)));
    const model = this.#model();
    const catalog = new ModuleCatalog({
      roots,
      loader: this.#options.loader,
      bindings: async () => ({
        ...this.#bindings(),
        tools,
        skills: skillMetadata,
        ...(model ? { model } : {}),
      }),
    });
    const source = await catalog.load(path);
    const after = (await catalog.list()).get(this.#resource.id);
    if (JSON.stringify(this.#resource) !== JSON.stringify(after)) {
      throw new Error('Machine declaration changed during preparation');
    }
    return { ...source, revision: this.#revision(source, tools, skillContents) };
  }
  async #tools(): Promise<ExecutableTool[]> {
    const selected = this.#pool.select(this.#settings['tools']);
    const artifactDirectory = resolve(this.#options.cwd, '.artifacts/tools');
    const { createCodemodeTool } = await import('@zf/tools');
    const codemode = createCodemodeTool({ tools: selected, artifactDirectory });
    const mode = this.#settings['codemode'];
    return mode && typeof mode === 'object' && !Array.isArray(mode) && mode['mode'] === 'only'
      ? [codemode]
      : [...selected, codemode];
  }
  #model(): MachineBindings['model'] {
    const selector = this.#settings['model'];
    return selector &&
      typeof selector === 'object' &&
      !Array.isArray(selector) &&
      typeof selector['provider'] === 'string' &&
      typeof selector['id'] === 'string'
      ? this.#options.runtime.select(selector['provider'], selector['id'])
      : undefined;
  }
  #bindings(): Pick<
    MachineBindings,
    'cwd' | 'config' | 'models' | 'onModelEvent' | 'onToolUpdate'
  > {
    return {
      cwd: this.#options.cwd,
      config: this.#settings,
      models: this.#options.runtime.models,
      ...(this.#options.onModelEvent ? { onModelEvent: this.#options.onModelEvent } : {}),
      ...(this.#options.onToolUpdate ? { onToolUpdate: this.#options.onToolUpdate } : {}),
    };
  }
  #revision(
    source: MachineSource,
    tools: readonly ExecutableTool[],
    skills: readonly Skill[],
  ): string {
    return createHash('sha256')
      .update(source.revision)
      .update(
        JSON.stringify({
          resolved: this.#resolved,
          skills,
          tools: tools.map((tool) => tool.declaration),
          declarations: this.#pool.fingerprint(),
        }),
      )
      .digest('hex');
  }
}
