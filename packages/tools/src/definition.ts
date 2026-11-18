// Adapted from Pi 1.5.2 coding-agent/src/core/tools/tool-definition-wrapper.ts (MIT).
// Copyright Mario Zechner and Earendil Works; see ../LICENSE and ../PROVENANCE.json.
import { scriptResults } from './script-results.ts';
import type {
  ImageContent,
  JsonValue,
  TextContent,
  Tool,
  ToolResultMessage,
} from '@earendil-works/pi-ai';
import type { ToolExecution } from '@zf/core';
import type { Static, TSchema } from 'typebox';
export { wrapToolDefinition } from './builtin-tool.ts';

export interface DefinitionResult<D = unknown> {
  content: (TextContent | ImageContent)[];
  details?: D | undefined;
  structuredContent?: unknown;
  isError?: boolean | undefined;
}
export interface ToolDefinition<S extends TSchema, D> {
  name: string;
  label: string;
  description: string;
  parameters: S;
  outputSchema?: TSchema;
  constrainedSampling?: Tool['constrainedSampling'];
  promptSnippet?: string | undefined;
  promptGuidelines?: string[] | undefined;
  prepareArguments?: (args: unknown) => Static<S>;
  execute: (input: DefinitionExecution<S, D>) => Promise<DefinitionResult<D>>;
}
export interface DefinitionExecution<S extends TSchema, D> {
  readonly toolCallId: string;
  readonly args: Static<S>;
  readonly signal: AbortSignal;
  readonly onUpdate?: ((result: DefinitionResult<D>) => void) | undefined;
}
export const jsonValue = (value: unknown): JsonValue =>
  JSON.parse(JSON.stringify(value)) as JsonValue;
export { errorText } from './error-text.ts';
export const resultMessage = (
  input: ToolExecution,
  result: DefinitionResult,
  durationMs?: number,
): ToolResultMessage => {
  const message: ToolResultMessage = {
    role: 'toolResult',
    toolCallId: input.call.id,
    toolName: input.call.name,
    content: result.content,
    isError: result.isError ?? false,
    timestamp: Date.now(),
    ...(durationMs === undefined ? {} : { durationMs }),
    ...(result.details === undefined ? {} : { details: jsonValue(result.details) }),
  };
  if (result.structuredContent !== undefined) {
    scriptResults.set(message, result.structuredContent);
  }
  return message;
};
