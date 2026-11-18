import { messageEntries } from './transcript-entries.ts';
import { wrapGutter } from './text-wrap.ts';

export interface SettledRows {
  readonly width: number;
  readonly rows: readonly string[];
  /** Older rows or messages were left out. */
  readonly truncated: boolean;
}

/** Owns the bounded settled history and its width-specific, newest-first row cache. */
export class TranscriptHistory {
  readonly #maxMessages: number;
  readonly #maxLines: number;
  #messages: readonly unknown[] = [];
  #hidden = 0;
  #cache: SettledRows | undefined;

  public constructor(maxMessages: number, maxLines: number) {
    this.#maxMessages = maxMessages;
    this.#maxLines = maxLines;
  }

  public setMessages(messages: readonly unknown[]): void {
    this.#messages = messages.slice(-this.#maxMessages);
    this.#hidden = messages.length - this.#messages.length;
    this.invalidate();
  }

  public invalidate(): void {
    this.#cache = undefined;
  }

  public render(width: number): SettledRows {
    if (this.#cache?.width === width) {
      return this.#cache;
    }
    this.#cache = this.#wrapHistory(width);
    return this.#cache;
  }

  #wrapHistory(width: number): SettledRows {
    const newestFirst: string[][] = [];
    let count = 0;
    let truncated = this.#hidden > 0;
    for (let index = this.#messages.length - 1; index >= 0; index -= 1) {
      if (count >= this.#maxLines) {
        truncated = true;
        break;
      }
      const rows = messageEntries(this.#messages[index]).flatMap((entry) =>
        wrapGutter(entry.lines, entry.gutter, { width, style: entry.style }),
      );
      newestFirst.push(rows);
      count += rows.length;
    }
    const all = newestFirst.reverse().flat();
    return {
      width,
      rows: all.slice(-this.#maxLines),
      truncated: truncated || all.length > this.#maxLines,
    };
  }
}
