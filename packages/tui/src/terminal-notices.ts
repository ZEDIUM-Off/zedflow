import type { TUI } from '@earendil-works/pi-tui';
import { yellow } from './style.ts';
import { sanitizeInline } from './text.ts';
import { TextBlock } from './text-block.ts';

export const GENERIC_HINT = 'Type a JSON event such as {"type":"INCREMENT"}, or /help.';
const AGENT_HINT = 'Type a prompt, or /help. Escape cancels the active turn.';
const NOTICE_LINES = 8;
const MAX_RAW_ERROR_CHARACTERS = 4000;

export const errorText = (error: unknown): string =>
  sanitizeInline(
    (error instanceof Error ? error.message : String(error)).slice(0, MAX_RAW_ERROR_CHARACTERS),
  );

/** Keeps the bounded diagnostic history silent once closing begins. */
export class TerminalNotices {
  readonly #tui: TUI;
  public readonly block = new TextBlock({ style: yellow, maxLines: NOTICE_LINES });
  #lines: string[];
  #closed = false;

  public constructor(tui: TUI, agent: boolean) {
    this.#tui = tui;
    this.#lines = [agent ? AGENT_HINT : GENERIC_HINT];
    this.block.setText(this.#lines.join('\n'));
  }

  public add(text: string): void {
    if (this.#closed) {
      return;
    }
    this.#lines = [...this.#lines, sanitizeInline(text, 400)].slice(-NOTICE_LINES);
    this.block.setText(this.#lines.join('\n'));
    this.#tui.requestRender();
  }

  public replace(lines: readonly string[]): void {
    this.#lines = [];
    for (const line of lines) {
      this.add(line);
    }
  }

  public close(): void {
    this.#closed = true;
  }
}
