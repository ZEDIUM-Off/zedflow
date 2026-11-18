import { SelectList } from '@earendil-works/pi-tui';
import type { Input } from '@earendil-works/pi-tui';
import type { AuthPrompt } from '@earendil-works/pi-ai';
import { LOGIN_CANCELLED } from './auth-panel.ts';
import type { AuthPanelOptions } from './auth-panel.ts';
import { createAuthInput } from './auth-input.ts';
import { SecretInput } from './secret-input.ts';
import { bold, dim } from './style.ts';
import { sanitizeLines } from './text.ts';
import { wrapLine } from './text-wrap.ts';

interface PendingPrompt {
  readonly prompt: AuthPrompt;
  readonly input: SecretInput | Input | SelectList;
  readonly cancel: () => void;
}

/** Owns one pending login prompt, its focus, abort listener and secret cleanup. */
export class AuthPrompts {
  readonly #options: AuthPanelOptions;
  #pending: PendingPrompt | undefined;
  #focused = false;

  public constructor(options: AuthPanelOptions) {
    this.#options = options;
  }

  public get focused(): boolean {
    return this.#focused;
  }

  public set focused(value: boolean) {
    this.#focused = value;
    this.#focusInput(this.#pending?.input);
  }

  public ask(prompt: AuthPrompt, signal: AbortSignal): Promise<string> {
    if (signal.aborted) {
      return Promise.reject(new Error(LOGIN_CANCELLED));
    }
    if (this.#pending) {
      return Promise.reject(new Error('Another login prompt is already shown'));
    }
    return new Promise<string>((resolve, reject) => {
      const finish = (settle: () => void): void => {
        signal.removeEventListener('abort', onAbort);
        this.#clearPending();
        settle();
      };
      const onAbort = (): void => finish(() => reject(new Error(LOGIN_CANCELLED)));
      const cancel = (): void => {
        finish(() => reject(new Error(LOGIN_CANCELLED)));
        this.#options.onCancel();
      };
      const input = createAuthInput(prompt, (value) => finish(() => resolve(value)), cancel);
      this.#pending = { prompt, input, cancel: onAbort };
      this.#focusInput(input);
      signal.addEventListener('abort', onAbort, { once: true });
      this.#options.requestRender();
    });
  }

  public close(): void {
    this.#pending?.cancel();
  }

  public handleInput(data: string): void {
    this.#pending?.input.handleInput?.(data);
  }

  public render(width: number): string[] {
    return this.#pending ? this.#renderPrompt(this.#pending, width) : [];
  }

  public invalidate(): void {
    this.#pending?.input.invalidate();
  }

  #renderPrompt(pending: PendingPrompt, width: number): string[] {
    const { prompt, input } = pending;
    const placeholder =
      'placeholder' in prompt && prompt.placeholder ? ` (${prompt.placeholder})` : '';
    const message = `${prompt.message}${placeholder}`;
    const heading = sanitizeLines(message).flatMap((line) =>
      wrapLine(line, Math.max(1, width - 1)),
    );
    const hint =
      prompt.type === 'select' ? 'Enter selects · Esc cancels' : 'Enter submits · Esc cancels';
    return ['', ...heading.map((row) => ` ${bold(row)}`), ...input.render(width), dim(` ${hint}`)];
  }

  #focusInput(input: SecretInput | Input | SelectList | undefined): void {
    if (input && !(input instanceof SelectList)) {
      input.focused = this.#focused;
    }
  }

  #clearPending(): void {
    const input = this.#pending?.input;
    if (input instanceof SecretInput) {
      input.clear();
    }
    this.#pending = undefined;
    this.#options.requestRender();
  }
}
