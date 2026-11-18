import type { Component } from '@earendil-works/pi-tui';
import { dim } from './style.ts';
import { fit } from './text.ts';
import { wrapGutter } from './text-wrap.ts';
import { isRecord } from './transcript-content.ts';
import { messageEntries, resultEntries } from './transcript-entries.ts';
import { TranscriptHistory } from './transcript-history.ts';

export interface TranscriptViewOptions {
  /** Newest messages parsed from the run view; older ones are summarized. Default 100. */
  readonly maxMessages?: number;
  /** Newest rendered rows kept after wrapping. Default 300. */
  readonly maxLines?: number;
}

/**
 * Bounded conversation view over Pi messages. Final messages come from the native run view; the
 * in-flight assistant message and tool result come from native model/tool observers.
 *
 * Settled messages are wrapped newest-first, only until the row bound is filled, and cached by
 * width; model and tool updates re-wrap just the in-flight parts.
 */
export class TranscriptView implements Component {
  readonly #history: TranscriptHistory;
  readonly #maxLines: number;
  #streaming: unknown;
  #progress: unknown;
  #cache: { readonly width: number; readonly rows: readonly string[] } | undefined;

  public constructor(options: TranscriptViewOptions = {}) {
    const maxMessages = options.maxMessages ?? 100;
    this.#maxLines = options.maxLines ?? 300;
    this.#history = new TranscriptHistory(maxMessages, this.#maxLines);
  }

  /** Replace the settled conversation; `messages` is untrusted run-view data. */
  public setMessages(messages: readonly unknown[]): void {
    this.#history.setMessages(messages);
    this.#cache = undefined;
  }

  /** Show the in-flight assistant message, or clear it with `undefined`. */
  public setStreaming(message: unknown): void {
    this.#streaming = message;
    this.#cache = undefined;
  }

  /** Show the in-flight tool result, or clear it with `undefined`. */
  public setToolProgress(result: unknown): void {
    this.#progress = result;
    this.#cache = undefined;
  }

  public render(width: number): string[] {
    if (width < 1) {
      return [];
    }
    if (this.#cache?.width !== width) {
      const settled = this.#history.render(width);
      const rows = [...settled.rows, ...this.#inflightRows(width)];
      const hidden = settled.truncated || rows.length > this.#maxLines;
      const shown = hidden
        ? [
            dim('… earlier output hidden'),
            ...rows.slice(rows.length - Math.max(0, this.#maxLines - 1)),
          ]
        : rows;
      this.#cache = { width, rows: shown.map((row) => fit(row, width)) };
    }
    return [...this.#cache.rows];
  }

  public invalidate(): void {
    this.#history.invalidate();
    this.#cache = undefined;
  }

  #inflightRows(width: number): string[] {
    return [
      ...(this.#streaming === undefined ? [] : messageEntries(this.#streaming)),
      ...(this.#progress === undefined
        ? []
        : resultEntries(isRecord(this.#progress) ? this.#progress : {})),
    ].flatMap((entry) => wrapGutter(entry.lines, entry.gutter, { width, style: entry.style }));
  }
}
