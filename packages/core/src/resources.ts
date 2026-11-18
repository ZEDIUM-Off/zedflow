import type {
  Api,
  AssistantMessageEvent,
  Model,
  Models,
  Tool,
  ToolCall,
  ToolResultMessage,
} from '@earendil-works/pi-ai';
import type { Json, SkillMetadata } from './contracts.ts';

export interface MachineManifest {
  readonly schemaVersion: 1;
  readonly name: string;
  readonly description: string;
  readonly entry: string;
  readonly defaults?: Readonly<Record<string, Json>>;
  readonly configSchema?: Readonly<Record<string, Json>>;
  readonly tools?: readonly string[];
  readonly ui?: string;
}

export interface MachineResource {
  readonly id: string;
  readonly directory: string;
  readonly scope: 'builtin' | 'global' | 'project';
  readonly manifest: MachineManifest;
}

export interface ToolExecution {
  readonly call: ToolCall;
  readonly signal: AbortSignal;
  /** Host has applied prepareArguments; the implementation still validates defensively. */
  readonly argumentsPrepared?: boolean;
  readonly onUpdate?: (result: ToolResultMessage) => void;
}

export interface ExecutableTool {
  readonly declaration: Tool;
  /** Pi-style pure argument normalization, applied before schema validation. */
  readonly prepareArguments?: (args: unknown) => unknown;
  readonly execute: (input: ToolExecution) => Promise<ToolResultMessage>;
}

// The model/provider interfaces are Pi AI's own types, not Zedflow substitutes.
export interface MachineBindings {
  readonly cwd: string;
  readonly config: Readonly<Record<string, Json>>;
  readonly models: Models;
  readonly model?: Model<Api>;
  readonly tools: readonly ExecutableTool[];
  readonly skills?: readonly SkillMetadata[];
  readonly onModelEvent?: (event: AssistantMessageEvent) => void;
  readonly onToolUpdate?: (result: ToolResultMessage) => void;
}

// The returned value is validated by the native engine at the loading seam.
export type MachineFactory = (bindings: MachineBindings) => unknown;
