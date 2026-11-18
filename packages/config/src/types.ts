import type { Json } from '@zf/core';

export type SettingsScope = 'global' | 'project';
/**
 * Open machine-specific JSON. Reserved fields: model {provider,id} selects a native Pi model;
 * tools is a string array; codemode.mode is 'on'|'only'; maxModelCalls is a positive integer.
 * Only files may contain machines[id] overrides. Credentials/auth fields are rejected recursively.
 * Dotted keys and prototype keys are rejected to keep dotted paths and provenance unambiguous.
 */
export type Settings = Record<string, Json>;
export type ConfigurationLayer =
  | 'program'
  | 'manifest'
  | 'global'
  | 'global:machine'
  | 'project'
  | 'project:machine'
  | 'overrides';
export interface ConfigurationOptions {
  readonly globalDirectory: string;
  readonly projectDirectory: string;
  readonly programDefaults?: Readonly<Settings>;
}
export interface ResolveOptions {
  readonly machineId: string;
  readonly defaults?: Readonly<Settings>;
  readonly overrides?: Readonly<Settings>;
}
/** Arrays replace, objects merge; provenance records leaf paths (including empty objects). */
export interface ResolvedConfiguration {
  readonly settings: Settings;
  readonly provenance: Readonly<Record<string, ConfigurationLayer>>;
}
