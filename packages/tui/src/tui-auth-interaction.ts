import type { AuthEvent, AuthInteraction, AuthPrompt } from '@earendil-works/pi-ai';
import { LOGIN_CANCELLED } from './auth-panel.ts';
import type { AuthPanel } from './auth-panel.ts';

export interface TuiAuthInteractionOptions {
  readonly panel: AuthPanel;
  /** Aborts the whole login flow. */
  readonly signal: AbortSignal;
}

const MIN_REDACTED_LENGTH = 4;

/**
 * Native Pi `AuthInteraction` backed by an `AuthPanel`. `Models.login` copies the interaction
 * with an object spread, so `prompt` and `notify` are own properties rather than prototype methods.
 */
export class TuiAuthInteraction implements AuthInteraction {
  public readonly signal: AbortSignal;
  readonly #panel: AuthPanel;
  readonly #secrets = new Set<string>();

  public constructor(options: TuiAuthInteractionOptions) {
    this.#panel = options.panel;
    this.signal = options.signal;
  }

  public readonly prompt = async (prompt: AuthPrompt): Promise<string> => {
    // A provider may cancel one prompt, e.g. a pasted code that lost the race to a browser callback.
    const signal = prompt.signal ? AbortSignal.any([this.signal, prompt.signal]) : this.signal;
    if (signal.aborted) {
      throw new Error(LOGIN_CANCELLED);
    }
    const answer = await this.#panel.ask(prompt, signal);
    if (prompt.type === 'secret' || prompt.type === 'manual_code') {
      this.#remember(answer);
    }
    return answer;
  };

  public readonly notify = (event: AuthEvent): void => {
    this.#panel.show(event);
  };

  /** Replace every entered secret or code fragment in `text`, e.g. inside a provider error. */
  public redact(text: string): string {
    let result = text;
    for (const secret of this.#secrets) {
      result = result.replaceAll(secret, '[redacted]');
    }
    return result;
  }

  #remember(answer: string): void {
    const fragments = [answer];
    try {
      fragments.push(...new URL(answer).searchParams.values());
    } catch {
      // A bare code is not a URL; the whole answer is already tracked.
    }
    for (const fragment of fragments) {
      if (fragment.length >= MIN_REDACTED_LENGTH) {
        this.#secrets.add(fragment);
      }
    }
  }
}
