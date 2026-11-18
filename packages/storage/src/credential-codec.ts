// Adapted from Pi 1.5.2 core/auth-storage.ts: credential validation.
// Copyright (c) 2025 Mario Zechner. MIT notice: ../UPSTREAM-LICENSE.
import type { Credential } from '@earendil-works/pi-ai';

const isRecord = (value: unknown): value is Record<string, unknown> =>
  Boolean(value) && typeof value === 'object' && !Array.isArray(value);
const record = (value: unknown): Record<string, unknown> => {
  if (!isRecord(value)) {
    throw new Error('Invalid credential file');
  }
  return value;
};
const isStringRecord = (value: Record<string, unknown>): value is Record<string, string> =>
  Object.values(value).every((item) => typeof item === 'string');
const apiKey = (data: Record<string, unknown>): Credential => {
  const key = data['key'];
  const env = data['env'];
  if (key !== undefined && typeof key !== 'string') {
    throw new Error('Invalid credential file');
  }
  const environment = env === undefined ? undefined : record(env);
  if (environment !== undefined && !isStringRecord(environment)) {
    throw new Error('Invalid credential file');
  }
  return {
    type: 'api_key',
    ...(key !== undefined ? { key } : {}),
    ...(environment !== undefined ? { env: environment } : {}),
  };
};
export const decodeCredential = (value: unknown): Credential => {
  const data = record(value);
  if (data['type'] === 'api_key') {
    return apiKey(data);
  }
  if (
    data['type'] === 'oauth' &&
    typeof data['access'] === 'string' &&
    typeof data['refresh'] === 'string' &&
    typeof data['expires'] === 'number' &&
    Number.isFinite(data['expires'])
  ) {
    return {
      ...data,
      type: 'oauth',
      access: data['access'],
      refresh: data['refresh'],
      expires: data['expires'],
    };
  }
  throw new Error('Invalid credential file');
};
export const decodeCredentials = (text: string): Map<string, Credential> => {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text.replace(/^\uFEFF/, ''));
  } catch {
    throw new Error('Invalid credential JSON');
  }
  return new Map(
    Object.entries(record(parsed)).map(([id, value]) => [id, decodeCredential(value)]),
  );
};
export const encodeCredentials = (data: Map<string, Credential>): string => {
  let content: string;
  try {
    content = JSON.stringify(Object.fromEntries(data), null, 2) + '\n';
  } catch {
    throw new Error('Credential cannot be serialized');
  }
  if (Buffer.byteLength(content) > 262144) {
    throw new Error('Credential file exceeds 256 KiB');
  }
  return content;
};
