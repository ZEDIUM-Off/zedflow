import {
  CURSOR_MARKER,
  decodeKittyPrintable,
  matchesKey,
  renderFakeCursor,
} from '@earendil-works/pi-tui';
import type { Component, Focusable } from '@earendil-works/pi-tui';
import { SecretPaste } from './secret-paste.ts';
import { fit } from './text.ts';

const MASK = '*';
const MAX_CHARACTERS = 8192;
// Bound on the text kept from one bracketed paste; the rest is scanned for the end marker only.
const MAX_PASTE_CHARACTERS = MAX_CHARACTERS * 4;
const PROMPT = '> ';
const graphemes = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

const printable = (text: string): string => {
  let result = '';
  for (const character of text) {
    const code = character.codePointAt(0) ?? 0;
    if (code >= 32 && code !== 0x7f && !(code >= 0x80 && code <= 0x9f)) {
      result += character;
    }
  }
  return result;
};

/**
 * Single-line input whose content is never rendered, logged or echoed: the screen shows only a
 * mask. The value leaves this component solely through `onSubmit`, and is discarded afterwards.
 */
export class SecretInput implements Component, Focusable {
  public focused = false;
  public onSubmit: ((value: string) => void) | undefined;
  public onCancel: (() => void) | undefined;
  #characters: string[] = [];
  readonly #paste = new SecretPaste(MAX_PASTE_CHARACTERS);

  public handleInput(data: string): void {
    const cancelled = matchesKey(data, 'escape') || matchesKey(data, 'ctrl+c');
    if (this.#paste.accepts(data)) {
      // An unterminated paste must not trap the user: cancel always works.
      if (cancelled && this.#paste.active) {
        this.#cancel();
      } else {
        this.#handlePaste(data);
      }
      return;
    }
    this.#handleKey(data, cancelled);
  }

  #handleKey(data: string, cancelled: boolean): void {
    if (cancelled) {
      this.#cancel();
    } else if (matchesKey(data, 'enter') || data === '\n') {
      this.#submit();
    } else if (matchesKey(data, 'backspace')) {
      this.#characters.pop();
    } else if (matchesKey(data, 'ctrl+u')) {
      this.#characters = [];
    } else {
      const decoded = decodeKittyPrintable(data);
      // Ignore other escape sequences, such as arrow keys, and control characters.
      this.#append(decoded ?? (data.startsWith('\u001b') ? '' : data));
    }
  }

  #cancel(): void {
    this.clear();
    this.onCancel?.();
  }

  /** Discard the typed value without submitting it. */
  public clear(): void {
    this.#characters = [];
    this.#paste.clear();
  }

  public render(width: number): string[] {
    if (width < 1) {
      return [];
    }
    const available = Math.max(0, width - PROMPT.length - 1);
    const count = this.#characters.length;
    const hidden = count > available;
    const mask = hidden ? `…${MASK.repeat(Math.max(0, available - 1))}` : MASK.repeat(count);
    const marker = this.focused ? CURSOR_MARKER : '';
    return [fit(`${PROMPT}${mask}${marker}${renderFakeCursor(' ')}`, width)];
  }

  public invalidate(): void {}

  #append(text: string): void {
    for (const { segment } of graphemes.segment(printable(text))) {
      if (this.#characters.length >= MAX_CHARACTERS) {
        return;
      }
      this.#characters.push(segment);
    }
  }

  #submit(): void {
    const value = this.#characters.join('');
    this.clear();
    this.onSubmit?.(value);
  }

  #handlePaste(data: string): void {
    const result = this.#paste.consume(data);
    if (result === undefined) {
      return;
    }
    this.#append(result.content.replace(/[\r\n]+$/u, ''));
    if (result.remaining !== '') {
      this.handleInput(result.remaining);
    }
  }
}
