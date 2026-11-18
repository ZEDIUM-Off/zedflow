import { createAssistantMessageEventStream } from '@earendil-works/pi-ai';
import type { Api, AssistantMessage, Model, Provider } from '@earendil-works/pi-ai';
import { openaiProvider } from '@earendil-works/pi-ai/providers/openai';

const message = (model: Model<Api>, aborted = false): AssistantMessage => ({
  role: 'assistant',
  content: [{ type: 'text', text: 'fixture answer' }],
  api: model.api,
  provider: model.provider,
  model: model.id,
  timestamp: Date.now(),
  stopReason: aborted ? 'aborted' : 'stop',
  usage: {
    input: 0,
    output: 0,
    cacheRead: 0,
    cacheWrite: 0,
    totalTokens: 0,
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
  },
});

export class StreamingProvider {
  #enter: (() => void) | undefined;
  readonly #entered = new Promise<void>((resolveEntered) => {
    this.#enter = resolveEntered;
  });
  #receivedKey = '';

  public get entered(): Promise<void> {
    return this.#entered;
  }
  public get receivedKey(): string {
    return this.#receivedKey;
  }
  public get provider(): Provider {
    return {
      ...openaiProvider(),
      streamSimple: (model, context, options) => this.stream(model, context, options),
    };
  }

  private stream(
    model: Parameters<Provider['streamSimple']>[0],
    context: Parameters<Provider['streamSimple']>[1],
    options: Parameters<Provider['streamSimple']>[2],
  ): ReturnType<Provider['streamSimple']> {
    const stream = createAssistantMessageEventStream();
    this.#receivedKey = options?.apiKey ?? '';
    const response = message(model);
    if (context.messages.length === 0) {
      stream.push({
        type: 'text_delta',
        contentIndex: 0,
        delta: 'fixture answer',
        partial: response,
      });
      stream.push({ type: 'done', reason: 'stop', message: response });
    } else {
      this.waitForAbort(stream, model, options?.signal);
    }
    return stream;
  }

  private waitForAbort(
    stream: ReturnType<typeof createAssistantMessageEventStream>,
    model: Model<Api>,
    signal: AbortSignal | undefined,
  ): void {
    const abort = (): void =>
      stream.push({ type: 'error', reason: 'aborted', error: message(model, true) });
    if (signal?.aborted) {
      abort();
    } else {
      signal?.addEventListener('abort', abort, { once: true });
    }
    this.#enter?.();
  }
}
