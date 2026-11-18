import { dim, plain } from './style.ts';
import { sanitizeInline } from './text.ts';
import type { Style } from './text.ts';
import { keepFirst } from './text-wrap.ts';
import type { Gutter } from './text-wrap.ts';

export interface Entry {
  readonly gutter: Gutter;
  readonly style: Style;
  readonly lines: readonly string[];
}

export const USER: Gutter = { first: '> ', rest: '  ' };
export const ASSISTANT: Gutter = { first: '  ', rest: '  ' };
export const THINKING: Gutter = { first: '~ ', rest: '  ' };
export const CALL: Gutter = { first: '* ', rest: '  ' };
export const RESULT: Gutter = { first: '= ', rest: '  ' };
export const FAILURE: Gutter = { first: '! ', rest: '  ' };

export const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);
export const blocks = (value: unknown): readonly unknown[] => (Array.isArray(value) ? value : []);
export const text = (value: unknown): string => (typeof value === 'string' ? value : '');

export const clip = (lines: readonly string[], limit: number): string[] =>
  keepFirst(lines, limit, plain);

// Images stay explicit placeholders: this view never writes terminal graphics sequences.
export const imageEntry = (block: Record<string, unknown>): Entry => {
  const kilobytes = Math.ceil((text(block['data']).length * 3) / 4 / 1024);
  return {
    gutter: RESULT,
    style: dim,
    lines: [
      `[image ${sanitizeInline(text(block['mimeType']), 40)}, about ${kilobytes} KiB; not rendered]`,
    ],
  };
};
