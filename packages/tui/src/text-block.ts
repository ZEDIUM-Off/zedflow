import type { Component } from '@earendil-works/pi-tui';
import { plain } from './style.ts';
import { sanitizeLines } from './text.ts';
import { wrapLine } from './text-wrap.ts';
import type { Style } from './text.ts';

export interface TextBlockOptions {
  readonly style?: Style;
  readonly maxLines?: number;
}

/** Bounded, wrapped block of untrusted text; an empty text renders no rows. */
export class TextBlock implements Component {
  readonly #style: Style;
  readonly #maxLines: number;
  #lines: readonly string[] = [];
  #cache: { readonly width: number; readonly rows: readonly string[] } | undefined;

  public constructor(options: TextBlockOptions = {}) {
    this.#style = options.style ?? plain;
    this.#maxLines = options.maxLines ?? 20;
  }

  public setText(text: string): void {
    this.#lines = text === '' ? [] : sanitizeLines(text).slice(0, this.#maxLines);
    this.#cache = undefined;
  }

  public render(width: number): string[] {
    if (this.#cache?.width !== width) {
      this.#cache = {
        width,
        rows: this.#lines.flatMap((line) => wrapLine(line, width).map((row) => this.#style(row))),
      };
    }
    return [...this.#cache.rows];
  }

  public invalidate(): void {
    this.#cache = undefined;
  }
}
