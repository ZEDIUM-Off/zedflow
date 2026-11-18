import { json } from '@zf/core';
import type { Json, Event } from '@zf/core';
export const object = (value: Json): value is { [key: string]: Json } =>
  !!value && typeof value === 'object' && !Array.isArray(value);
export const parseJson = (text: string): Json => {
  try {
    const value: unknown = JSON.parse(text);
    return json(value);
  } catch {
    throw new Error('Expected valid JSON');
  }
};

export const validEvent = (value: Json): value is Event =>
  object(value) && typeof value['type'] === 'string';
