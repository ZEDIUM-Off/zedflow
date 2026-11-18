import { visibleWidth, wrapTextWithAnsi } from '@earendil-works/pi-tui';
import { fit } from './text.ts';
import type { Style } from './text.ts';

/** Wrap one sanitized line to `width` columns; every row is guaranteed to fit. */
export const wrapLine = (line: string, width: number): string[] => {
  if (width < 1) {
    return [];
  }
  const rows = line === '' ? [''] : wrapTextWithAnsi(line, width);
  return (rows.length === 0 ? [''] : rows).map((row) => fit(row, width));
};

export interface Gutter {
  readonly first: string;
  readonly rest: string;
}

/** Wrap sanitized lines under a fixed-width gutter and style each resulting row. */
export const wrapGutter = (
  lines: readonly string[],
  gutter: Gutter,
  options: { readonly width: number; readonly style: Style },
): string[] => {
  const gutterWidth = Math.max(visibleWidth(gutter.first), visibleWidth(gutter.rest));
  const bodyWidth = options.width - gutterWidth;
  if (bodyWidth < 1) {
    // No room for a gutter: show each line truncated.
    return lines.map((line) => fit(options.style(line), Math.max(0, options.width)));
  }
  const rows: string[] = [];
  for (const line of lines) {
    for (const row of wrapLine(line, bodyWidth)) {
      rows.push(
        fit(
          `${rows.length === 0 ? gutter.first : gutter.rest}${options.style(row)}`,
          options.width,
        ),
      );
    }
  }
  return rows;
};

/** Keep the last `limit` rows, replacing the hidden prefix by one explicit marker row. */
export const keepLast = (rows: readonly string[], limit: number, marker: Style): string[] => {
  if (rows.length <= limit) {
    return [...rows];
  }
  const hidden = rows.length - limit + 1;
  return [marker(`… ${hidden} earlier lines hidden`), ...rows.slice(rows.length - limit + 1)];
};

/** Keep the first `limit` rows, replacing the hidden suffix by one explicit marker row. */
export const keepFirst = (rows: readonly string[], limit: number, marker: Style): string[] => {
  if (rows.length <= limit) {
    return [...rows];
  }
  return [...rows.slice(0, limit - 1), marker(`… ${rows.length - limit + 1} more lines`)];
};
