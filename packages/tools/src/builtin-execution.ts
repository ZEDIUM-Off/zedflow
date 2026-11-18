// Adapted from Pi 1.5.2 coding-agent/src/core/tools/tool-definition-wrapper.ts (MIT).
// Copyright Mario Zechner and Earendil Works; see ../LICENSE and ../PROVENANCE.json.
import { validateToolArguments } from '@earendil-works/pi-ai';
import type { JsonObject, Tool, ToolResultMessage } from '@earendil-works/pi-ai';
import type { ToolExecution } from '@zf/core';
import type { Static, TSchema } from 'typebox';
import { errorText, resultMessage } from './definition.ts';
import type { DefinitionResult, ToolDefinition } from './definition.ts';

/** Owns the update window of one built-in call, including abort and finalization. */
export class BuiltinExecution<S extends TSchema, D> {
  readonly #definition: ToolDefinition<S, D>;
  readonly #input: ToolExecution;
  readonly #startedAt = performance.now();
  #finalized = false;

  public constructor(definition: ToolDefinition<S, D>, input: ToolExecution) {
    this.#definition = definition;
    this.#input = input;
  }

  public async execute(declaration: Tool): Promise<ToolResultMessage> {
    try {
      this.#input.signal.throwIfAborted();
      const args = this.#arguments(declaration);
      const result = await this.#definition.execute({
        toolCallId: this.#input.call.id,
        args,
        signal: this.#input.signal,
        onUpdate: this.#input.onUpdate ? (update) => this.#publish(update) : undefined,
      });
      this.#input.signal.throwIfAborted();
      return resultMessage(this.#input, result, performance.now() - this.#startedAt);
    } catch (error) {
      return resultMessage(
        this.#input,
        { content: [{ type: 'text', text: errorText(error) }], isError: true },
        performance.now() - this.#startedAt,
      );
    } finally {
      this.#finalized = true;
    }
  }

  #arguments(declaration: Tool): Static<S> {
    if (this.#input.call.name !== declaration.name) {
      throw new Error(`Expected tool ${declaration.name}, got ${this.#input.call.name}`);
    }
    const copied = structuredClone(this.#input.call.arguments);
    const prepared =
      !this.#input.argumentsPrepared && this.#definition.prepareArguments
        ? this.#definition.prepareArguments(copied)
        : copied;
    const args: unknown = validateToolArguments(declaration, {
      ...this.#input.call,
      arguments: prepared as JsonObject,
    });
    return args as Static<S>;
  }

  #publish(update: DefinitionResult<D>): void {
    if (!this.#finalized && !this.#input.signal.aborted) {
      this.#input.onUpdate?.(
        resultMessage(this.#input, { ...update, structuredContent: undefined }),
      );
    }
  }
}
