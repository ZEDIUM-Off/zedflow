import { isObject } from './guards.ts';
import { assertAllowedKey } from './key-policy.ts';
import type { Settings } from './types.ts';

const validateJsonObject = (value: Settings): void => {
  const prototype: unknown = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new Error('Settings must contain only JSON objects');
  }
  for (const [key, child] of Object.entries(value)) {
    assertAllowedKey(key);
    validateJson(child);
  }
};

const isJsonScalar = (value: unknown): boolean =>
  value === null ||
  typeof value === 'boolean' ||
  typeof value === 'string' ||
  (typeof value === 'number' && Number.isFinite(value));

export const validateJson = (value: unknown): void => {
  if (isJsonScalar(value)) {
    return;
  }
  if (Array.isArray(value)) {
    for (const child of value) {
      validateJson(child);
    }
    return;
  }
  if (!isObject(value)) {
    throw new Error('Settings must contain only JSON values');
  }
  validateJsonObject(value);
};
