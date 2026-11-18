import type { Tool, ToolCall, ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool, MachineBindings } from '@zf/core';
import { createAsyncLogic, types } from 'xstate';
import { ToolCallExecution } from './tool-call-execution.ts';

/** The declared tool list and dispatch map share one unique-name registry per generation. */
export class AgentTools {
  readonly #tools: Map<string, ExecutableTool>;
  readonly #onUpdate: MachineBindings['onToolUpdate'];

  public readonly logic = createAsyncLogic({
    schemas: { input: types<ToolCall>(), output: types<ToolResultMessage>() },
    run: ({ input, signal }) => this.#execute(input, signal),
  });

  public constructor(bindings: MachineBindings) {
    this.#tools = new Map(bindings.tools.map((tool) => [tool.declaration.name, tool]));
    if (this.#tools.size !== bindings.tools.length) {
      throw new Error('Agent tool names must be unique');
    }
    this.#onUpdate = bindings.onToolUpdate;
  }

  public declarations(): Tool[] {
    return [...this.#tools.values()].map((tool) => tool.declaration);
  }

  #execute(call: ToolCall, signal: AbortSignal): Promise<ToolResultMessage> {
    return new ToolCallExecution(call, signal, this.#onUpdate).run(this.#tools.get(call.name));
  }
}
