import { isObject } from './guards.ts';
import { SettingsMerger } from './settings-merger.ts';
import { validateMachineId, validateSettings } from './settings-validation.ts';
import type {
  ConfigurationLayer,
  ResolvedConfiguration,
  Settings,
  SettingsScope,
} from './types.ts';

/** Layers the settings of one machine from lowest to highest precedence. */
export class MachineResolution {
  readonly #machineId: string;
  readonly #merger = new SettingsMerger();

  public constructor(machineId: string, programDefaults: Settings) {
    validateMachineId(machineId);
    this.#machineId = machineId;
    this.#merger.apply(programDefaults, 'program');
  }

  public applyManifest(defaults: Readonly<Settings>): void {
    this.#merger.apply(validateSettings(defaults, false), 'manifest');
  }

  public applyScope(scope: SettingsScope, settings: Settings): void {
    const { machines, ...shared } = settings;
    this.#merger.apply(shared, scope);
    if (isObject(machines)) {
      this.#applyMachine(machines[this.#machineId], `${scope}:machine`);
    }
  }

  public applyOverrides(overrides: Readonly<Settings>): void {
    this.#merger.apply(validateSettings(overrides, false), 'overrides');
  }

  public result(): ResolvedConfiguration {
    const result = this.#merger.result();
    const model = result.settings['model'];
    if (isObject(model) && (!model['provider'] || !model['id'])) {
      throw new Error('Effective model requires both provider and id');
    }
    return result;
  }

  #applyMachine(machine: unknown, layer: ConfigurationLayer): void {
    if (isObject(machine)) {
      this.#merger.apply(machine, layer);
    }
  }
}
