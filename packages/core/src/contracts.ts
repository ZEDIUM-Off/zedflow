export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export interface Event {
  readonly type: string;
  readonly [key: string]: Json;
}
export interface MachineSource {
  readonly path: string;
  readonly revision: string;
  readonly module: unknown;
}
export interface Definition {
  readonly id: string;
  readonly version: string;
  readonly opaque: unknown;
}
export interface Frame {
  readonly type: string;
  readonly actor: string;
  readonly parent?: string;
  readonly data: Json;
}
export interface Execution {
  readonly start: () => void;
  readonly stop: () => void;
  readonly send: (event: Event) => void;
  readonly view: () => Json;
  readonly checkpoint: () => Json;
  readonly checkpointStatus?: () => 'ready' | 'busy' | 'unsupported';
}
export interface Engine {
  readonly modules?: Record<string, unknown>;
  readonly identify: (module: unknown) => Definition;
  readonly prepare: (
    definition: Definition,
    observe: (frame: Frame) => void,
    checkpoint?: Json,
  ) => Execution;
}
export interface Loader {
  readonly load: (path: string, directory?: string) => Promise<MachineSource>;
}
export interface RecordEntry {
  readonly seq: number;
  readonly kind: string;
  readonly data: Json;
}
export interface RecordStore {
  readonly append: (runId: string, kind: string, data: Json) => void;
  readonly read: (runId: string) => RecordEntry[];
}
export type ReloadStatus = 'waiting' | 'preparing' | 'applied' | 'refused';
export interface ReloadOptions {
  readonly wait?: boolean;
  /** Hosts should supply and abort this signal before stopping during preparation.
   * Without it, a trusted loader that never settles cannot be cancelled. */
  readonly signal?: AbortSignal;
}
export interface RunUpdate {
  readonly view: Json;
  readonly generation: number;
  readonly stopped: boolean;
  readonly reload?: ReloadStatus;
}
export interface RunService {
  readonly launch: (path: string, origin?: string) => Promise<string>;
  readonly send: (runId: string, event: Event) => void;
  readonly view: (runId: string) => Json;
  readonly records: (runId: string) => RecordEntry[];
  readonly reload: (runId: string, options?: ReloadOptions) => Promise<void>;
  readonly subscribe: (runId: string, listener: (update: RunUpdate) => void) => () => void;
  readonly stop: (runId: string) => void;
}
export interface ToolDeclaration {
  readonly name: string;
  readonly description?: string;
  readonly inputSchema: Json;
  readonly execution:
    | { readonly builtin: string }
    | { readonly command: string; readonly args: readonly string[] };
}
export interface SkillMetadata {
  readonly name: string;
  readonly description: string;
  readonly path: string;
}
export interface Skill extends SkillMetadata {
  readonly content: string;
}
