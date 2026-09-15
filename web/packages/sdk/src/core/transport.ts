import type { z } from 'zod';
import type { JsonValue } from './json.js';

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
export type QueryScalar = string | number | boolean;
export type QueryValue = QueryScalar | readonly QueryScalar[] | undefined;

export interface RequestDescriptor {
  operation: string;
  method: HttpMethod;
  /** Path relative to the API base URL. Encode dynamic path segments. */
  path: string;
  query?: Readonly<Record<string, QueryValue>>;
  body?: JsonValue;
  headers?: Readonly<Record<string, string>>;
  signal?: AbortSignal;
}

export interface Transport {
  json<S extends z.ZodType>(request: RequestDescriptor, schema: S): Promise<z.output<S>>;
  bytes(request: RequestDescriptor): Promise<Uint8Array>;
}
