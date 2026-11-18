import { createAssistantMessageEventStream } from '@earendil-works/pi-ai';
import type { Provider, TranscriptContext } from '@earendil-works/pi-ai';
import { openaiProvider } from '@earendil-works/pi-ai/providers/openai';
import { assistantMessage, toolCallContent } from './messages.ts';

class ScriptedProvider {
  readonly #contexts: TranscriptContext[] = [];
  readonly #mode: 'direct' | 'only';
  readonly #hold: ((finish: () => void) => void) | undefined;

  public constructor(mode: 'direct' | 'only', hold?: (finish: () => void) => void) {
    this.#mode = mode;
    this.#hold = hold;
  }

  public get contexts(): TranscriptContext[] {
    return this.#contexts;
  }

  public get provider(): Provider {
    return { ...openaiProvider(), streamSimple: (model, context) => this.#stream(model, context) };
  }

  #stream(
    model: Parameters<Provider['streamSimple']>[0],
    context: TranscriptContext,
  ): ReturnType<Provider['streamSimple']> {
    this.#contexts.push(structuredClone(context));
    const first = this.#contexts.length === 1;
    const message = assistantMessage(
      model,
      first ? toolCallContent(this.#mode) : [{ type: 'text', text: 'Native turn complete' }],
      first ? 'toolUse' : 'stop',
    );
    const stream = createAssistantMessageEventStream();
    if (this.#contexts.length === 2 && this.#hold) {
      stream.push({ type: 'start', partial: message });
      stream.push({
        type: 'text_delta',
        contentIndex: 0,
        delta: 'Native turn complete',
        partial: message,
      });
      this.#hold(() => stream.push({ type: 'done', reason: 'stop', message }));
    } else {
      stream.push({ type: 'done', reason: first ? 'toolUse' : 'stop', message });
    }
    return stream;
  }
}

export const scriptedProvider = (
  mode: 'direct' | 'only',
  options: { hold?: (finish: () => void) => void } = {},
): ScriptedProvider => new ScriptedProvider(mode, options.hold);
