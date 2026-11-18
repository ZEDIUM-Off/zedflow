import { createAssistantMessageEventStream } from '@earendil-works/pi-ai';
import type { Provider, TranscriptContext } from '@earendil-works/pi-ai';
import { openaiProvider } from '@earendil-works/pi-ai/providers/openai';
import { assistantMessage } from './messages.ts';

export class DeferredReloadProvider {
  readonly #contexts: TranscriptContext[] = [];
  readonly #entered: Promise<void>;
  #enter: (() => void) | undefined;
  #finish: (() => void) | undefined;
  #aborted = false;

  public constructor() {
    this.#entered = new Promise<void>((resolveEntered) => {
      this.#enter = resolveEntered;
    });
  }

  public get contexts(): TranscriptContext[] {
    return this.#contexts;
  }
  public get entered(): Promise<void> {
    return this.#entered;
  }
  public get aborted(): boolean {
    return this.#aborted;
  }

  public get provider(): Provider {
    return {
      ...openaiProvider(),
      streamSimple: (model, context, options) => this.#stream(model, context, options),
    };
  }

  public finish(): void {
    this.#finish?.();
  }

  #stream(
    model: Parameters<Provider['streamSimple']>[0],
    context: TranscriptContext,
    options: Parameters<Provider['streamSimple']>[2],
  ): ReturnType<Provider['streamSimple']> {
    this.#contexts.push(structuredClone(context));
    const index = this.#contexts.length;
    const stream = createAssistantMessageEventStream();
    const message = assistantMessage(model, [{ type: 'text', text: `answer ${index}` }], 'stop');
    if (index === 1) {
      options?.signal?.addEventListener(
        'abort',
        () => {
          this.#aborted = true;
        },
        { once: true },
      );
      this.#finish = () => stream.push({ type: 'done', reason: 'stop', message });
      this.#enter?.();
    } else {
      stream.push({ type: 'done', reason: 'stop', message });
    }
    return stream;
  }
}
