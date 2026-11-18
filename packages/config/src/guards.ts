import type { Settings } from './types.ts';

export const isObject = (value: unknown): value is Settings =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

export const hasErrorCode = (error: unknown, code: string): boolean =>
  error instanceof Error && 'code' in error && error.code === code;

export const isMissingFile = (error: unknown): boolean => hasErrorCode(error, 'ENOENT');
