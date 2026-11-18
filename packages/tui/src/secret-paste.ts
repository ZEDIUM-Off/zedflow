const PASTE_START = '\u001b[200~';
const PASTE_END = '\u001b[201~';

interface CompletedPaste {
  readonly content: string;
  readonly remaining: string;
}

/** Bounded paste content plus a partial end marker; overflow is scanned, never retained. */
export class SecretPaste {
  readonly #limit: number;
  #paste: { text: string; pending: string } | undefined;

  public constructor(limit: number) {
    this.#limit = limit;
  }

  public get active(): boolean {
    return this.#paste !== undefined;
  }

  public accepts(data: string): boolean {
    return this.active || data.includes(PASTE_START);
  }

  public clear(): void {
    this.#paste = undefined;
  }

  public consume(data: string): CompletedPaste | undefined {
    const paste = this.#paste ?? { text: '', pending: '' };
    const scan = paste.pending + (this.active ? data : data.replace(PASTE_START, ''));
    const end = scan.indexOf(PASTE_END);
    if (end === -1) {
      // Keep a possible partial end marker aside; commit the rest within the bound.
      const keep = Math.max(0, scan.length - (PASTE_END.length - 1));
      paste.text = this.#bounded(paste.text + scan.slice(0, keep));
      paste.pending = scan.slice(keep);
      this.#paste = paste;
      return undefined;
    }
    this.clear();
    return {
      content: this.#bounded(paste.text + scan.slice(0, end)),
      remaining: scan.slice(end + PASTE_END.length),
    };
  }

  #bounded(text: string): string {
    if (text.length <= this.#limit) {
      return text;
    }
    const code = text.charCodeAt(this.#limit - 1);
    // Never leave half of a surrogate pair at the cut.
    return text.slice(0, code >= 0xd800 && code <= 0xdbff ? this.#limit - 1 : this.#limit);
  }
}
