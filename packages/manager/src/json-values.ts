import { readFile } from 'node:fs/promises';
import type { Json } from '@zf/core';

export const object = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);
export const jsonObject = (value: unknown): value is Record<string, Json> => {
  if (!object(value)) {
    return false;
  }
  return Object.entries(value).every(
    ([key, child]) => !['__proto__', 'constructor', 'prototype'].includes(key) && isJson(child),
  );
};
const isJson = (value: unknown): value is Json =>
  value === null ||
  typeof value === 'string' ||
  typeof value === 'boolean' ||
  (typeof value === 'number' && Number.isFinite(value)) ||
  (Array.isArray(value) ? value.every(isJson) : jsonObject(value));

export const readJson = async (path: string): Promise<unknown> =>
  JSON.parse(await readFile(path, 'utf8')) as unknown;
