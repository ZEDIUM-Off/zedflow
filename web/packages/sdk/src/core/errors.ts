import type { z } from 'zod';

export type ErrorKind = 'network' | 'http' | 'json' | 'response-validation' | 'request-validation' | 'aborted';

export class ZedflowError extends Error {
  readonly kind: ErrorKind;
  readonly operation: string;
  constructor(kind: ErrorKind, operation: string, message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = new.target.name;
    this.kind = kind;
    this.operation = operation;
  }
}

export class NetworkError extends ZedflowError {
  constructor(operation: string, cause: unknown) {
    super('network', operation, `Network failure during ${operation}`, { cause });
  }
}

export class HttpError extends ZedflowError {
  readonly status: number;
  readonly rawBody: string;
  readonly body: unknown;
  constructor(operation: string, status: number, rawBody: string, body: unknown) {
    super('http', operation, `HTTP ${status} during ${operation}`);
    this.status = status;
    this.rawBody = rawBody;
    this.body = body;
  }
}

export class JsonDecodeError extends ZedflowError {
  constructor(operation: string, cause: unknown) {
    super('json', operation, `Invalid JSON response for ${operation}`, { cause });
  }
}

export class ResponseValidationError extends ZedflowError {
  readonly issues: z.ZodError['issues'];
  constructor(operation: string, cause: z.ZodError) {
    super('response-validation', operation, `Invalid response contract for ${operation}`, { cause });
    this.issues = cause.issues;
  }
}

export class RequestValidationError extends ZedflowError {
  constructor(operation: string, cause: unknown) {
    super('request-validation', operation, `Invalid request for ${operation}`, { cause });
  }
}

export class RequestAbortedError extends ZedflowError {
  constructor(operation: string, cause: unknown) {
    super('aborted', operation, `Request cancelled during ${operation}`, { cause });
  }
}
