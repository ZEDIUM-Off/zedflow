import { join, resolve } from 'node:path';
import type { Json } from '@zf/core';
import { PROGRAM_DEFAULTS } from './defaults.ts';
import { validateJson } from './json-validation.ts';
import { MachineResolution } from './machine-resolution.ts';
import { SettingsFile } from './settings-file.ts';
import { SettingsKeyPath } from './settings-key-path.ts';
import { SettingsMerger } from './settings-merger.ts';
import { validateSettings } from './settings-validation.ts';
import type {
  ConfigurationOptions,
  ResolveOptions,
  ResolvedConfiguration,
  Settings,
  SettingsScope,
} from './types.ts';

/** Async JSON settings only; provider models and credentials remain owned by Pi AI. */
export class Configuration {
  readonly #files: Readonly<Record<SettingsScope, SettingsFile>>;
  readonly #defaults: Settings;

  public constructor(options: ConfigurationOptions) {
    this.#files = {
      global: new SettingsFile(join(resolve(options.globalDirectory), 'settings.json')),
      project: new SettingsFile(
        join(resolve(options.projectDirectory), '.zedflow', 'settings.json'),
      ),
    };
    const defaults = new SettingsMerger();
    defaults.apply(validateSettings(PROGRAM_DEFAULTS, false), 'program');
    defaults.apply(validateSettings(options.programDefaults ?? {}, false), 'program');
    this.#defaults = defaults.result().settings;
  }

  public async read(scope: SettingsScope): Promise<Settings> {
    return await this.#file(scope).read();
  }

  public async set(scope: SettingsScope, dottedPath: string, value: Json): Promise<void> {
    validateJson(value);
    const key = new SettingsKeyPath(dottedPath);
    await this.#file(scope).set(key, value);
  }

  public async unset(scope: SettingsScope, dottedPath: string): Promise<void> {
    const key = new SettingsKeyPath(dottedPath);
    await this.#file(scope).unset(key);
  }

  public async resolve(options: ResolveOptions): Promise<ResolvedConfiguration> {
    const resolution = new MachineResolution(options.machineId, this.#defaults);
    resolution.applyManifest(options.defaults ?? {});
    resolution.applyScope('global', await this.read('global'));
    resolution.applyScope('project', await this.read('project'));
    resolution.applyOverrides(options.overrides ?? {});
    return resolution.result();
  }

  #file(scope: SettingsScope): SettingsFile {
    if (scope !== 'global' && scope !== 'project') {
      throw new Error(`Invalid settings scope: ${String(scope)}`);
    }
    return this.#files[scope];
  }
}
