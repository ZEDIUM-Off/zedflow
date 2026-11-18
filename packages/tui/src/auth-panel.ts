import type { AuthEvent, AuthPrompt } from '@earendil-works/pi-ai';
import type { Component, Focusable } from '@earendil-works/pi-tui';
import { AuthNotices } from './auth-notices.ts';
import { AuthPrompts } from './auth-prompts.ts';
import { bold, cyan, dim } from './style.ts';
import { fit, sanitizeInline } from './text.ts';
import { wrapGutter } from './text-wrap.ts';

export const LOGIN_CANCELLED = 'Login cancelled';

export interface AuthPanelOptions {
  readonly requestRender: () => void;
  /** Called after the panel rejected its pending prompt because the user pressed Escape or Ctrl+C. */
  readonly onCancel: () => void;
}

/**
 * Login surface of the terminal UI: provider notices plus one prompt at a time. Secret and
 * pasted-code prompts are masked; their text never reaches the rendered rows.
 */
export class AuthPanel implements Component, Focusable {
  readonly #options: AuthPanelOptions;
  #title = 'Sign in';
  readonly #notices = new AuthNotices();
  readonly #prompts: AuthPrompts;

  public constructor(options: AuthPanelOptions) {
    this.#options = options;
    this.#prompts = new AuthPrompts(options);
  }

  public get focused(): boolean {
    return this.#prompts.focused;
  }

  public set focused(value: boolean) {
    this.#prompts.focused = value;
  }

  public setTitle(title: string): void {
    this.#title = sanitizeInline(title, 80);
    this.#options.requestRender();
  }

  /** Show a provider notification: information, sign-in URL, device code or progress. */
  public show(event: AuthEvent): void {
    this.#notices.show(event);
    this.#options.requestRender();
  }

  /**
   * Ask the user. Resolves with the entered text or the selected option id; rejects when `signal`
   * aborts, when the user cancels, or when another prompt is already shown.
   */
  public ask(prompt: AuthPrompt, signal: AbortSignal): Promise<string> {
    return this.#prompts.ask(prompt, signal);
  }

  /** Reject the pending prompt and drop the panel's content. */
  public close(): void {
    this.#prompts.close();
    this.#notices.clear();
  }

  public handleInput(data: string): void {
    this.#prompts.handleInput(data);
    this.#options.requestRender();
  }

  public render(width: number): string[] {
    if (width < 1) {
      return [];
    }
    const rule = dim('─'.repeat(width));
    const rows = [
      rule,
      ...wrapGutter(
        [this.#title],
        { first: ' ', rest: ' ' },
        { width, style: (t) => bold(cyan(t)) },
      ),
      ...this.#notices.render(width),
      ...this.#prompts.render(width),
      rule,
    ];
    return rows.map((row) => fit(row, width));
  }

  public invalidate(): void {
    this.#prompts.invalidate();
  }
}
