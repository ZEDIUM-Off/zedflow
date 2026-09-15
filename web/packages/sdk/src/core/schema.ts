import type { z } from 'zod';
import { RequestValidationError, ResponseValidationError } from './errors.js';

export function validateResponse<S extends z.ZodType>(operation: string, schema: S, value: unknown): z.output<S> {
  const result = schema.safeParse(value);
  if (!result.success) throw new ResponseValidationError(operation, result.error);
  return result.data;
}

export function validateInput<S extends z.ZodType>(operation: string, schema: S, value: unknown): z.output<S> {
  const result = schema.safeParse(value);
  if (!result.success) throw new RequestValidationError(operation, result.error);
  return result.data;
}
