import type { Json } from './contracts.ts';
export const json = (value: unknown): Json => {
  const text = JSON.stringify(value, (_key, item: unknown) => {
    if (typeof item === 'function' || typeof item === 'symbol' || typeof item === 'bigint') {
      throw new Error('Unsupported JSON payload');
    }
    if (typeof item === 'number' && !Number.isFinite(item)) {
      throw new Error('Non-finite JSON payload');
    }
    if (item instanceof Map || item instanceof Set) {
      throw new Error('Unsupported JSON collection');
    }
    return item;
  });
  if (text === undefined) {
    throw new Error('Missing JSON payload');
  }
  return JSON.parse(text) as Json;
};
