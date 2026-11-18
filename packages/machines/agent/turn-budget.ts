import type { AgentContext } from './machine.ts';
import type { AgentSettings } from './settings.ts';

/** Admits a whole batch only when it fits and leaves room for the follow-up model request. */
export class TurnBudget {
  readonly #maxModelCalls: number;
  readonly #maxToolCalls: number;

  public constructor(settings: AgentSettings) {
    this.#maxModelCalls = settings.maxModelCalls;
    this.#maxToolCalls = settings.maxToolCalls;
  }

  public route(context: AgentContext): {
    target: 'ready' | 'failed' | 'tool';
    context?: AgentContext;
  } {
    if (context.pending.length === 0) {
      return { target: 'ready' };
    }
    const failure =
      context.modelCalls >= this.#maxModelCalls
        ? 'Model-call budget exhausted'
        : context.pending.length > this.#maxToolCalls - context.toolCalls
          ? 'Tool-call budget exhausted'
          : null;
    return failure
      ? { target: 'failed', context: { ...context, pending: [], failure } }
      : { target: 'tool' };
  }
}
