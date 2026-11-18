import type { Provider, TranscriptContext } from '@earendil-works/pi-ai';
import { openaiProvider } from '@earendil-works/pi-ai/providers/openai';
import { expect } from 'vite-plus/test';

export class ConversationProvider {
  readonly #steps: readonly Provider['streamSimple'][];
  readonly #requests: TranscriptContext[] = [];

  public constructor(steps: readonly Provider['streamSimple'][]) {
    this.#steps = steps;
  }

  public get requests(): TranscriptContext[] {
    return this.#requests;
  }

  public get provider(): Provider {
    return {
      ...openaiProvider(),
      auth: {
        apiKey: {
          name: 'Offline fixture',
          resolve: async () => ({ auth: { apiKey: 'synthetic-conversation-key' } }),
        },
      },
      streamSimple: (model, context, options) => this.stream(model, context, options),
    };
  }

  private stream(
    model: Parameters<Provider['streamSimple']>[0],
    context: Parameters<Provider['streamSimple']>[1],
    options: Parameters<Provider['streamSimple']>[2],
  ): ReturnType<Provider['streamSimple']> {
    const step = this.#steps[this.#requests.length];
    this.#requests.push(structuredClone(context));
    expect(options?.apiKey).toBe('synthetic-conversation-key');
    if (!step) {
      throw new Error('Unexpected native provider request');
    }
    return step(model, context, options);
  }
}
