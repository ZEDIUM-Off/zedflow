import { test } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';
import { createClient, jsonObjectSchema, HttpError, JsonDecodeError, NetworkError, ResponseValidationError, RequestValidationError, RequestAbortedError } from '@zedflow/sdk';

test('HTTP requests preserve open JSON, API prefix, query and configured protocol', async () => {
  const value = { id: 'run-1', state: { unknown: { nested: [true, null, 'é🙂'] } } };
  let observed: { url: string; init?: RequestInit } | undefined;
  const client = createClient({
    baseUrl: 'https://example.test/service/api', protocol: 1,
    fetch: async (url, init) => {
      observed = { url: String(url), init };
      return new Response(JSON.stringify(value));
    },
  });
  const output = await client.transport.json({
    operation: 'runs.get', method: 'GET', path: '/runs/run-1',
    query: { workspaceId: 'a/b + é', revision: 2 },
  }, z.looseObject({ id: z.string(), state: jsonObjectSchema }));
  assert.deepEqual(output, value);
  assert.equal(observed?.url, 'https://example.test/service/api/runs/run-1?workspaceId=a%2Fb+%2B+%C3%A9&revision=2');
  assert.equal(new Headers(observed?.init?.headers).get('X-Zedflow-Protocol'), '1');
});

test('invalid inputs and escaped API paths are rejected before sending', async () => {
  let sent = 0;
  const client = createClient({ baseUrl: 'https://example.test/api', protocol: 1,
    fetch: async () => { sent++; return new Response('{}'); },
  });
  for (const body of [{ value: Infinity }, { value: undefined }, { value: new Date() }]) {
    await assert.rejects(client.transport.json({ operation: 'runs.create', method: 'POST', path: 'runs', body }, jsonObjectSchema), RequestValidationError);
  }
  for (const path of ['https://another.test/api', '//another.test/api', '../outside', '%2e%2e/outside', 'runs#hidden']) {
    await assert.rejects(client.transport.json({ operation: 'runs.get', method: 'GET', path }, jsonObjectSchema), RequestValidationError);
  }
  await assert.rejects(client.transport.json({ operation: 'runs.get', method: 'GET', path: 'runs', query: { limit: NaN } }, jsonObjectSchema), RequestValidationError);
  assert.equal(sent, 0);
});

test('cancellation preserves its reason and never becomes a network failure', async () => {
  const controller = new AbortController();
  const reason = new Error('view closed');
  controller.abort(reason);
  let sent = 0;
  const client = createClient({ baseUrl: 'https://example.test/api', protocol: 1,
    fetch: async () => { sent++; return new Response('{}'); },
  });
  await assert.rejects(client.transport.json({ operation: 'runs.get', method: 'GET', path: 'runs', signal: controller.signal }, jsonObjectSchema), error => {
    assert.ok(error instanceof RequestAbortedError);
    assert.equal(error.cause, reason);
    return true;
  });
  assert.equal(sent, 0);
  const pending = new AbortController();
  const pendingClient = createClient({ baseUrl: 'https://example.test/api', protocol: 1,
    fetch: async (_, init) => {
      assert.equal(init?.signal, pending.signal);
      pending.abort(reason);
      // An adapter may finish even after cancellation; this must not publish a result.
      return new Response('{}');
    },
  });
  await assert.rejects(pendingClient.transport.bytes({ operation: 'raw', method: 'GET', path: 'raw', signal: pending.signal }), RequestAbortedError);
});

test('raw reads return the original bytes including whitespace and non-UTF8 media', async () => {
  const raw = new Uint8Array([32, 123, 10, 34, 195, 169, 34, 58, 49, 125, 13, 10, 255, 0]);
  let accept: string | null = null;
  const client = createClient({ baseUrl: 'https://example.test/api', protocol: 1,
    fetch: async (_, init) => {
      accept = new Headers(init?.headers).get('accept');
      return new Response(raw);
    },
  });
  assert.deepEqual(await client.transport.bytes({ operation: 'inference.raw', method: 'GET', path: 'raw/call-1' }), raw);
  assert.equal(accept, 'application/octet-stream');
});

test('network, HTTP, invalid JSON and invalid response contracts remain distinct', async () => {
  const request = { operation: 'runs.get', method: 'GET' as const, path: 'runs/one' };
  const schema = z.strictObject({ id: z.string() });
  const clientFor = (fetch: typeof globalThis.fetch) => createClient({ baseUrl: 'https://example.test/api', protocol: 1, fetch });
  await assert.rejects(clientFor(async () => { throw new TypeError('offline'); }).transport.json(request, schema), NetworkError);
  await assert.rejects(clientFor(async () => new Response('{')).transport.json(request, schema), JsonDecodeError);
  await assert.rejects(clientFor(async () => new Response('{"id":1}')).transport.json(request, schema), ResponseValidationError);
  const diagnostics = { error: 'Conflict', diagnostics: [{ code: 'revision_conflict', path: 'run' }] };
  await assert.rejects(clientFor(async () => new Response(JSON.stringify(diagnostics), { status: 409 })).transport.json(request, schema), error => {
    assert.ok(error instanceof HttpError);
    assert.equal(error.status, 409);
    assert.deepEqual(error.body, diagnostics);
    return true;
  });
  await assert.rejects(clientFor(async () => new Response('proxy unavailable', { status: 502 })).transport.json(request, schema), error => {
    assert.ok(error instanceof HttpError);
    assert.equal(error.status, 502);
    assert.equal(error.rawBody, 'proxy unavailable');
    return true;
  });
});
