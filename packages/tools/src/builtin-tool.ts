// Adapted from Pi 1.5.2 coding-agent/src/core/tools/tool-definition-wrapper.ts (MIT).
// Copyright Mario Zechner and Earendil Works; see ../LICENSE and ../PROVENANCE.json.
import type { Tool, ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool, ToolExecution } from '@zf/core';
import type { TSchema } from 'typebox';
import type { ToolDefinition } from './definition.ts';
import { BuiltinExecution } from './builtin-execution.ts';
class BuiltinTool<S extends TSchema, D> implements ExecutableTool {
  public readonly declaration: Tool;
  readonly #definition: ToolDefinition<S, D>;

  public constructor(definition: ToolDefinition<S, D>) {
    this.#definition = definition;
    this.declaration = {
      name: definition.name,
      description: definition.description,
      parameters: definition.parameters,
      ...(definition.constrainedSampling === undefined
        ? {}
        : { constrainedSampling: definition.constrainedSampling }),
    };
  }

  public prepareArguments(args: unknown): unknown {
    const copy = structuredClone(args);
    return this.#definition.prepareArguments ? this.#definition.prepareArguments(copy) : copy;
  }

  public async execute(input: ToolExecution): Promise<ToolResultMessage> {
    return new BuiltinExecution(this.#definition, input).execute(this.declaration);
  }
}

export const wrapToolDefinition = <S extends TSchema, D>(
  definition: ToolDefinition<S, D>,
): ExecutableTool => new BuiltinTool(definition);
