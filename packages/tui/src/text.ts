import {
  replaceControlCharacters,
  stripTerminalSequences,
  truncateToWidth,
  visibleWidth,
} from '@earendil-works/pi-tui';

// Bidirectional formatting characters can visually reorder untrusted text.
const BIDI_FORMATTING = /[\u061C\u200E\u200F\u202A-\u202E\u2066-\u2069]/gu;
const LINE_BREAK = /\r\n|\r|\n/u;
const TAB = /\t/gu;

export const MAX_UNTRUSTED_CHARACTERS = 100_000;

export type Style = (text: string) => string;

const boundCharacters = (text: string, limit: number): string => {
  if (text.length <= limit) {
    return text;
  }
  const code = text.charCodeAt(limit - 1);
  // Never leave half of a surrogate pair at the cut.
  return text.slice(0, code >= 0xd800 && code <= 0xdbff ? limit - 1 : limit);
};

// Pi replaces control characters, newlines included, by a space; apply it per line to keep structure.
export const sanitizeLine = (text: string): string =>
  replaceControlCharacters(
    stripTerminalSequences(text).replace(BIDI_FORMATTING, '').replace(TAB, '  '),
  );

/** Split untrusted text into lines free of terminal control sequences, bounded in size. */
export const sanitizeLines = (text: string, limit = MAX_UNTRUSTED_CHARACTERS): string[] =>
  stripTerminalSequences(boundCharacters(text, limit)).split(LINE_BREAK).map(sanitizeLine);

/** One sanitized line for inline diagnostics, truncated to `limit` characters. */
export const sanitizeInline = (text: string, limit = 200): string => {
  const line = sanitizeLine(boundCharacters(text, limit * 4)).trim();
  return line.length > limit ? `${boundCharacters(line, limit - 1)}…` : line;
};

export const fit = (row: string, width: number): string =>
  visibleWidth(row) > width ? truncateToWidth(row, width, '') : row;
