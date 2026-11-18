// Adapted from Pi 1.5.2 cli/auth-terminal.ts. Copyright Mario Zechner.
// MIT notice: ../UPSTREAM-LICENSE.
import { StringDecoder } from 'node:string_decoder';
import { terminalText } from './terminal-text.ts';

/** Owns masked input and restores the exact terminal mode on every completion path. */
export class SecretPrompt {
  readonly #abort: AbortController;
  readonly #signal: AbortSignal;
  readonly #stdin = process.stdin;
  readonly #decoder = new StringDecoder('utf8');
  readonly #dataListener = this.#data.bind(this);
  readonly #abortListener = this.#cancel.bind(this);
  #raw = false;
  #flowing = false;
  #value = '';
  #finished = false;
  #resolve: ((value: string) => void) | undefined;
  #reject: ((error: Error) => void) | undefined;
  public constructor(abort: AbortController, signal: AbortSignal) {
    this.#abort = abort;
    this.#signal = signal;
  }
  public read(message: string): Promise<string> {
    this.#signal.throwIfAborted();
    this.#raw = this.#stdin.isRaw;
    this.#flowing = this.#stdin.readableFlowing === true;
    return new Promise((resolve, reject) => {
      this.#resolve = resolve;
      this.#reject = reject;
      this.#listen(message);
    });
  }
  #listen(message: string): void {
    // Disable echo and install handlers before advertising that secret input is ready.
    this.#stdin.setRawMode(true);
    this.#stdin.on('data', this.#dataListener);
    this.#stdin.once('end', this.#abortListener);
    this.#stdin.once('error', this.#abortListener);
    this.#signal.addEventListener('abort', this.#abortListener, { once: true });
    process.stderr.write(terminalText(message));
    this.#stdin.resume();
    if (this.#signal.aborted) {
      this.#cancel();
    }
  }
  #finish(): void {
    if (this.#finished) {
      return;
    }
    this.#finished = true;
    this.#stdin.off('data', this.#dataListener);
    this.#stdin.off('end', this.#abortListener);
    this.#stdin.off('error', this.#abortListener);
    this.#signal.removeEventListener('abort', this.#abortListener);
    this.#stdin.setRawMode(this.#raw);
    if (!this.#flowing) {
      this.#stdin.pause();
    }
    process.stderr.write('\n');
  }
  #cancel(): void {
    this.#finish();
    this.#value = '';
    this.#reject?.(new Error('Login cancelled'));
  }
  #data(chunk: Buffer | string): void {
    const text = typeof chunk === 'string' ? chunk : this.#decoder.write(chunk);
    if (text.startsWith('\u001b')) {
      return;
    }
    for (const char of text) {
      if (this.#character(char)) {
        return;
      }
    }
  }
  #character(char: string): boolean {
    if (char === '\r' || char === '\n') {
      this.#accept();
      return true;
    }
    if (char === '\u0003' || char === '\u0004') {
      this.#abort.abort();
      this.#cancel();
      return true;
    }
    this.#edit(char);
    if (this.#value.length > 16384) {
      this.#cancel();
      return true;
    }
    return false;
  }
  #accept(): void {
    const result = this.#value;
    this.#value = '';
    this.#finish();
    this.#resolve?.(result);
  }
  #edit(char: string): void {
    if (char === '\u007f' || char === '\b') {
      this.#value = Array.from(
        new Intl.Segmenter('und', { granularity: 'grapheme' }).segment(this.#value),
        (part) => part.segment,
      )
        .slice(0, -1)
        .join('');
    } else if (char >= ' ' && !/[\u007f-\u009f]/.test(char)) {
      this.#value += char;
    }
  }
}
