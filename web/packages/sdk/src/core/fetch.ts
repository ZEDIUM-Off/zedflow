import { z } from 'zod';
import type { RequestDescriptor, Transport } from './transport.js';
import { HttpError, JsonDecodeError, NetworkError, RequestAbortedError, RequestValidationError } from './errors.js';
import { validateInput, validateResponse } from './schema.js';
import { jsonValueSchema } from './json.js';

const queryScalar = z.union([z.string(), z.number(), z.boolean()]);
const querySchema = z.record(z.string(), z.union([queryScalar, z.array(queryScalar), z.undefined()]));
const descriptorSchema = z.strictObject({
  operation: z.string().min(1),
  method: z.enum(['GET', 'POST', 'PUT', 'PATCH', 'DELETE']),
  path: z.string(),
});

export type Fetch = (url: string, init?: RequestInit) => Promise<Response>;
export interface FetchTransportOptions {
  /** Absolute API URL, including the API prefix (for example /api). */
  baseUrl: string;
  protocol: number;
  fetch?: Fetch;
  headers?: Readonly<Record<string, string>>;
}

export class FetchTransport implements Transport {
  private readonly baseUrl: URL;
  private readonly fetchImplementation: Fetch;
  private readonly options: FetchTransportOptions;

  constructor(options: FetchTransportOptions) {
    if (!Number.isSafeInteger(options.protocol) || options.protocol < 1) throw new TypeError('A positive protocol version is required');
    this.options = { ...options, ...(options.headers ? { headers: { ...options.headers } } : {}) };
    this.baseUrl = new URL(`${options.baseUrl.replace(/\/+$/, '')}/`);
    if (!['http:', 'https:'].includes(this.baseUrl.protocol) || this.baseUrl.search || this.baseUrl.hash || this.baseUrl.username || this.baseUrl.password) {
      throw new TypeError('An absolute HTTP API base URL without query, fragment or credentials is required');
    }
    this.fetchImplementation = options.fetch ?? ((url, init) => globalThis.fetch(url, init));
  }

  async json<S extends z.ZodType>(request: RequestDescriptor, schema: S): Promise<z.output<S>> {
    const bytes = await this.exchange(request, 'application/json');
    let value: unknown;
    try {
      value = JSON.parse(new TextDecoder().decode(bytes));
    } catch (cause) {
      throw new JsonDecodeError(request.operation, cause);
    }
    return validateResponse(request.operation, schema, value);
  }

  bytes(request: RequestDescriptor): Promise<Uint8Array> {
    return this.exchange(request, 'application/octet-stream');
  }

  private async exchange(request: RequestDescriptor, accept: string): Promise<Uint8Array> {
    if (request.signal?.aborted) throw new RequestAbortedError(request.operation, request.signal.reason);
    validateInput(request.operation, descriptorSchema, { operation: request.operation, method: request.method, path: request.path });
    const url = new URL(request.path.replace(/^\//, ''), this.baseUrl);
    if (/^[a-z][a-z\d+.-]*:/i.test(request.path) || request.path.startsWith('//') || url.origin !== this.baseUrl.origin || !url.pathname.startsWith(this.baseUrl.pathname) || url.hash) {
      throw new RequestValidationError(request.operation, new Error('Request path must remain within the configured API base'));
    }
    for (const [key, value] of Object.entries(validateInput(request.operation, querySchema, request.query ?? {}))) {
      if (value === undefined) continue;
      for (const item of Array.isArray(value) ? value : [value]) url.searchParams.append(key, String(item));
    }
    const headers = new Headers({ ...this.options.headers, ...request.headers });
    headers.set('X-Zedflow-Protocol', String(this.options.protocol));
    headers.set('Accept', accept);
    const init: RequestInit = { method: request.method, headers };
    if (request.signal) init.signal = request.signal;
    if (request.body !== undefined) {
      headers.set('Content-Type', 'application/json');
      init.body = JSON.stringify(validateInput(request.operation, jsonValueSchema, request.body));
    }
    let response: Response;
    let bytes: Uint8Array;
    try {
      response = await this.fetchImplementation(url.href, init);
      bytes = new Uint8Array(await response.arrayBuffer());
    } catch (cause) {
      if (request.signal?.aborted || (cause instanceof Error && cause.name === 'AbortError')) {
        throw new RequestAbortedError(request.operation, request.signal?.aborted ? request.signal.reason : cause);
      }
      throw new NetworkError(request.operation, cause);
    }
    if (request.signal?.aborted) throw new RequestAbortedError(request.operation, request.signal.reason);
    if (!response.ok) {
      const text = new TextDecoder().decode(bytes);
      let body: unknown;
      try { body = JSON.parse(text); } catch { /* Preserve a non-JSON error as rawBody. */ }
      throw new HttpError(request.operation, response.status, text, body);
    }
    return bytes;
  }
}
