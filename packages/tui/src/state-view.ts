import type { Component } from '@earendil-works/pi-tui';
import type { Json } from '@zf/core';
import { bold, dim } from './style.ts';
import { fit, sanitizeInline, sanitizeLines } from './text.ts';
import { keepFirst, wrapGutter } from './text-wrap.ts';

export interface StateViewOptions {
  readonly maxLines?: number;
}

const isRecord = (value: Json): value is { [key: string]: Json } =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const stringify = (value: Json | undefined, space?: number): string => {
  try {
    return JSON.stringify(value, null, space) ?? 'undefined';
  } catch {
    return '[view is not serializable]';
  }
};

const summaryLines = (record: { [key: string]: Json } | undefined): string[] => {
  const summary: string[] = [];
  if (record && record['value'] !== undefined) {
    const value = record['value'];
    summary.push(`state  ${sanitizeInline(typeof value === 'string' ? value : stringify(value))}`);
  }
  if (record && record['status'] !== undefined) {
    const status = record['status'];
    summary.push(
      `status ${sanitizeInline(typeof status === 'string' ? status : stringify(status))}`,
    );
  }
  return summary;
};

/**
 * Generic presentation of a native run view: state value, status and JSON context.
 * Works for any machine; the native view shape `{value, context, status}` is optional.
 */
export class StateView implements Component {
  readonly #maxLines: number;
  #summary: readonly string[] = [];
  #context: readonly string[] = [];
  #cache: { readonly width: number; readonly rows: readonly string[] } | undefined;

  public constructor(options: StateViewOptions = {}) {
    this.#maxLines = options.maxLines ?? 200;
  }

  public setView(view: Json): void {
    const record = isRecord(view) ? view : undefined;
    const context = record ? record['context'] : view;
    this.#summary = summaryLines(record);
    this.#context = context === undefined ? [] : sanitizeLines(stringify(context, 2));
    this.#cache = undefined;
  }

  public render(width: number): string[] {
    if (width < 1) {
      return [];
    }
    if (this.#cache?.width !== width) {
      const summary = this.#summary.map((line) => fit(bold(line), width));
      const context = this.#context.flatMap((line) =>
        wrapGutter([line], { first: '  ', rest: '  ' }, { width, style: (text) => text }),
      );
      this.#cache = {
        width,
        rows: [
          ...summary,
          ...(context.length > 0 ? [dim('context')] : []),
          ...keepFirst(context, this.#maxLines, dim),
        ].map((row) => fit(row, width)),
      };
    }
    return [...this.#cache.rows];
  }

  public invalidate(): void {
    this.#cache = undefined;
  }
}
