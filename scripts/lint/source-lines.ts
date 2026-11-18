import type { Comment, SourceCode, Span } from 'vite-plus/lint/plugins';

/**
 * Lines of one file that hold code. Blank lines and lines that hold only comments are
 * skipped, as `max-lines` and `max-lines-per-function` do with `skipBlankLines` and
 * `skipComments`.
 */
export class SourceLines {
  readonly #code: readonly boolean[];

  public constructor(source: SourceCode) {
    const lines = [...source.lines];
    for (const comment of source.getAllComments()) {
      this.#blank(lines, comment);
    }
    this.#code = lines.map((line) => line.trim() !== '');
  }

  /** Number of code lines from the first to the last line of `node`. */
  public count(node: Span): number {
    return this.#code.slice(node.loc.start.line - 1, node.loc.end.line).filter(Boolean).length;
  }

  #blank(lines: string[], comment: Comment): void {
    const { start, end } = comment.loc;
    for (let line = start.line; line <= end.line; line += 1) {
      const text = lines[line - 1] ?? '';
      const from = line === start.line ? start.column : 0;
      const to = line === end.line ? end.column : text.length;
      lines[line - 1] = text.slice(0, from) + ' '.repeat(to - from) + text.slice(to);
    }
  }
}
