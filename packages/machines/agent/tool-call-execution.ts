import { validateToolArguments } from '@earendil-works/pi-ai';
import type { JsonObject, ToolCall, ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool, MachineBindings } from '@zf/core';
import { historyMessage, toolError } from './history.ts';

/** One invocation owns update admission until it settles or its actor is aborted. */
export class ToolCallExecution {
  readonly #call: ToolCall;
  readonly #signal: AbortSignal;
  readonly #onUpdate: MachineBindings['onToolUpdate'];
  #finalized = false;

  public constructor(
    call: ToolCall,
    signal: AbortSignal,
    onUpdate: MachineBindings['onToolUpdate'],
  ) {
    this.#call = call;
    this.#signal = signal;
    this.#onUpdate = onUpdate;
  }

  public async run(tool: ExecutableTool | undefined): Promise<ToolResultMessage> {
    try {
      this.#signal.throwIfAborted();
      if (!tool) {
        throw new Error(`Tool ${this.#call.name} not found`);
      }
      const call = this.#prepare(tool);
      this.#signal.throwIfAborted();
      const result = await tool.execute({
        call,
        argumentsPrepared: true,
        signal: this.#signal,
        onUpdate: (update) => this.#update(update),
      });
      this.#signal.throwIfAborted();
      return this.#result(result);
    } catch (error) {
      return toolError(this.#call, error);
    } finally {
      this.#finalized = true;
    }
  }

  #prepare(tool: ExecutableTool): ToolCall {
    // Pi prepares before validation. Clone arguments so normalizers cannot rewrite history.
    const argumentsCopy: unknown = structuredClone(this.#call.arguments);
    const prepared: unknown = tool.prepareArguments
      ? tool.prepareArguments(argumentsCopy)
      : argumentsCopy;
    const argumentsValue: unknown = validateToolArguments(tool.declaration, {
      ...this.#call,
      arguments: prepared as JsonObject,
    });
    return { ...this.#call, arguments: argumentsValue as JsonObject };
  }

  #update(update: ToolResultMessage): void {
    if (!this.#signal.aborted && !this.#finalized) {
      const onUpdate = this.#onUpdate;
      onUpdate?.(update);
    }
  }

  #result(result: ToolResultMessage): ToolResultMessage {
    if (result.toolCallId !== this.#call.id || result.toolName !== this.#call.name) {
      throw new Error('Tool result does not match its call');
    }
    return historyMessage(result);
  }
}
