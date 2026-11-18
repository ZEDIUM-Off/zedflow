import type { AssistantMessage, Message, Tool } from '@earendil-works/pi-ai';
import type { MachineBindings } from '@zf/core';
import { createAsyncLogic, types } from 'xstate';
import { historyMessage } from './history.ts';
import type { AgentSettings } from './settings.ts';

/** Binds each model request to this generation's model, instructions, tools and event sink. */
export class ModelRequest {
  readonly #models: MachineBindings['models'];
  readonly #model: AgentSettings['model'];
  readonly #systemPrompt: string;
  readonly #declarations: Tool[];
  readonly #onEvent: MachineBindings['onModelEvent'];

  public readonly logic = createAsyncLogic({
    schemas: { input: types<Message[]>(), output: types<AssistantMessage>() },
    run: ({ input, signal }) => this.#run(input, signal),
  });

  public constructor(bindings: MachineBindings, settings: AgentSettings, declarations: Tool[]) {
    this.#models = bindings.models;
    this.#model = settings.model;
    this.#systemPrompt = settings.systemPrompt;
    this.#declarations = declarations;
    this.#onEvent = bindings.onModelEvent;
  }

  async #run(messages: Message[], signal: AbortSignal): Promise<AssistantMessage> {
    signal.throwIfAborted();
    const stream = this.#models.streamSimple(
      this.#model,
      { messages, systemPrompt: this.#systemPrompt, tools: this.#declarations },
      { signal },
    );
    const onEvent = this.#onEvent;
    for await (const event of stream) {
      signal.throwIfAborted();
      onEvent?.(event);
    }
    signal.throwIfAborted();
    return historyMessage(await stream.result());
  }
}
