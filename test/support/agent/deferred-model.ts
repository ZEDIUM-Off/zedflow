import { createAssistantMessageEventStream } from '@earendil-works/pi-ai';
import type { AssistantMessage, Provider } from '@earendil-works/pi-ai';
import { message } from './messages.ts';

export class DeferredModel {
  readonly #pending = createAssistantMessageEventStream();
  #signal: AbortSignal | undefined;
  #lateMessage: AssistantMessage | undefined;

  public get signal(): AbortSignal | undefined {
    return this.#signal;
  }

  public stream(): Provider['streamSimple'] {
    return (model, _context, options) => {
      this.#signal = options?.signal;
      this.#lateMessage = message(model, [{ type: 'text', text: 'old response' }]);
      this.#pending.push({ type: 'start', partial: this.#lateMessage });
      this.#pending.push({
        type: 'text_delta',
        contentIndex: 0,
        delta: 'before abort',
        partial: this.#lateMessage,
      });
      return this.#pending;
    };
  }

  public finishAfterAbort(): void {
    if (!this.#lateMessage) {
      throw new Error('Model did not start');
    }
    this.#pending.push({
      type: 'text_delta',
      contentIndex: 0,
      delta: 'after abort',
      partial: this.#lateMessage,
    });
    this.#pending.push({ type: 'done', reason: 'stop', message: this.#lateMessage });
  }
}
