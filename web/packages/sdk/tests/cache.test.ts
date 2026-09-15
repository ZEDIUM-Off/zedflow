import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as sdk from '../dist/index.js';
const key = (query = {}, revision: string | number = 1) => ({ workspaceId: 'ws', runId: 'run', kind: 'tool', id: 'tool', revision, query });
const deferred = <T>() => { let resolve!: (value: T) => void; let reject!: (error: unknown) => void; const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };

test('cache canonical query identity, revisions, instance isolation and concurrent deduplication', async () => {
  const cache = new sdk.DetailCache<number>({ capacity: 2 });
  const waiting = deferred<number>();
  let calls = 0;
  const one = cache.load(key({ b: 2, a: 1 }), () => { calls++; return waiting.promise; });
  const two = cache.load(key({ a: 1, b: 2 }), () => { calls++; return Promise.resolve(99); });
  assert.equal(one, two);
  waiting.resolve(42);
  assert.equal(await one, 42);
  assert.equal(calls, 1);
  assert.equal(cache.entry(key({ a: 1, b: 2 }))?.value, 42);
  assert.equal(cache.entry(key({ a: 1 })), undefined);
  assert.equal(cache.entry(key({ a: 1, b: 2 }, '1')), undefined);
  assert.equal(new sdk.DetailCache().entry(key({ a: 1, b: 2 })), undefined);
  await cache.load({ ...key(), workspaceId: 'other' }, async () => 5);
  await cache.load({ ...key(), runId: 'other' }, async () => 6);
  assert.equal(cache.size, 2);
  assert.equal(cache.entry(key({ b: 2, a: 1 })), undefined);
  cache.dispose();
});

test('pending eviction and disposal reject immediately and cannot populate from late success or failure', async () => {
  const cache = new sdk.DetailCache<number>({ capacity: 1 });
  const old = deferred<number>();
  let signal: AbortSignal | undefined;
  const first = cache.load(key(), current => { signal = current; return old.promise; });
  const rejected = assert.rejects(first, sdk.RequestAbortedError);
  const next = cache.load(key({}, 2), async () => 2);
  await rejected;
  assert.equal(signal!.aborted, true);
  assert.equal(await next, 2);
  old.resolve(9);
  await Promise.resolve();
  assert.equal(cache.entry(key()), undefined);
  const late = deferred<number>();
  const final = cache.load(key({}, 3), () => late.promise);
  const disposed = assert.rejects(final, sdk.RequestAbortedError);
  cache.dispose();
  await disposed;
  late.reject(new Error('late transport failure'));
  await Promise.resolve();
  assert.equal(cache.size, 0);
});

test('typed detail facade uses query in load and entry and captures scope before awaiting', async () => {
  const responses = new Map<string, ReturnType<typeof deferred<Response>>>();
  const client = sdk.createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async url => {
    const response = deferred<Response>(); responses.set(url, response); return response.promise;
  } });
  const scope = { runId: 'first', workspaceId: 'ws' };
  const request = { ...scope, kind: 'event-history' as const, revision: 4, query: { after: 3 } };
  const first = client.details.load(request);
  assert.equal(client.details.load(request), first);
  request.runId = 'second'; request.query.after = 6;
  const second = client.details.load(request);
  assert.equal(responses.size, 2);
  for (const [url, response] of responses) response.resolve(new Response(JSON.stringify({ events: [], cursor: url.includes('/first/') ? 4 : 7, hasMore: false })));
  assert.equal((await first).cursor, 4); assert.equal((await second).cursor, 7);
  assert.equal(client.details.entry({ ...scope, kind: 'event-history', revision: 4, query: { after: 3 } })?.value?.cursor, 4);
  assert.equal(client.details.entry({ ...scope, kind: 'event-history', revision: 4, query: { after: 6 } }), undefined);
  assert.equal(client.details.entry({ ...scope, workspaceId: 'other', kind: 'event-history', revision: 4, query: { after: 3 } }), undefined);
  client.dispose(); assert.equal(client.details.size, 0);
});

test('detail identity mismatch is rejected and a failed entry can be retried', async () => {
  let id = 'wrong';
  const client = sdk.createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async () => new Response(JSON.stringify({ callId: id, output: 'text' })) });
  const request = { runId: 'run', workspaceId: 'ws', kind: 'tools' as const, id: 'tool', revision: 1 };
  await assert.rejects(client.details.load(request), sdk.SyncProtocolError);
  assert.ok(client.details.entry(request)?.error);
  id = 'tool'; assert.equal((await client.details.load(request)).callId, 'tool');
  client.dispose();
});

test('exact definitions are pinned and bounded; diagnostics/latest definitions remain refreshable', async () => {
  let calls = 0;
  const client = sdk.createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async url => {
    calls++;
    const query = new URL(url).searchParams;
    return new Response(JSON.stringify(query.get('nodePath') === 'missing'
      ? { exact: false, runId: 'run', diagnostic: { code: 'missing', message: 'Not captured' } }
      : { exact: true, runId: 'run', instance: 'instance', nodePath: query.get('nodePath') ?? '', occurrenceId: query.get('occurrenceId'), key: 'flow', hash: 'hash', source: 'fn main(){}', composition: { id: 'flow', name: 'Flow', revision: 0, nodes: [], edges: [] } }));
  } });
  const definitions = new sdk.ExecutedDefinitions(client.runs, { capacity: 1 });
  const pinned = { runId: 'run', workspaceId: 'ws', query: { nodePath: 'node', occurrenceId: 'one' } };
  const first = definitions.load(pinned);
  assert.equal(definitions.load(pinned), first);
  await first; await definitions.load(pinned); assert.equal(calls, 1);
  const missing = { ...pinned, query: { nodePath: 'missing', occurrenceId: 'one' } };
  await definitions.load(missing); await definitions.load(missing); assert.equal(calls, 3);
  assert.equal(definitions.entry(missing), undefined);
  assert.equal(definitions.entry(pinned), undefined);
  await definitions.load({ ...pinned, query: { nodePath: 'node' } });
  await definitions.load({ ...pinned, query: { nodePath: 'node' } }); assert.equal(calls, 5);
  await assert.rejects(definitions.load({ ...pinned, runId: 'other' }), sdk.SyncProtocolError);
  definitions.dispose(); client.dispose();
});

test('cache retries even falsy rejection values', async () => {
  const cache = new sdk.DetailCache<number>();
  await assert.rejects(cache.load(key(), async () => { throw null; }), error => error === null);
  assert.equal(await cache.load(key(), async () => 3), 3);
  cache.dispose();
});

test('new APIs retain their precise types when consumed outside the workspace without DOM access', async () => {
  const { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const { dirname, join, resolve } = await import('node:path');
  const { createRequire } = await import('node:module');
  const { execFileSync } = await import('node:child_process');
  const require = createRequire(import.meta.url), directory = mkdtempSync(join(tmpdir(), 'zedflow-sync-consumer-'));
  try {
    const installed = join(directory, 'node_modules/@zedflow/sdk'); mkdirSync(installed, { recursive: true });
    cpSync(resolve('dist'), join(installed, 'dist'), { recursive: true }); cpSync(resolve('package.json'), join(installed, 'package.json'));
    cpSync(dirname(require.resolve('zod/package.json')), join(directory, 'node_modules/zod'), { recursive: true });
    writeFileSync(join(directory, 'package.json'), '{"type":"module"}');
    writeFileSync(join(directory, 'consumer.ts'), `import { createClient, RunProjection, type ToolActivity, type RunFollower } from '@zedflow/sdk';
      const client = createClient({baseUrl:'https://fixture.test/api',protocol:1});
      const tool: Promise<ToolActivity> = client.details.load({runId:'r', workspaceId:'w', kind:'tools', id:'t', revision:2});
      const loaded: ToolActivity | undefined = client.details.entry({runId:'r',workspaceId:'w',kind:'tools',id:'t'})?.value;
      // @ts-expect-error A tool identity is mandatory.
      client.details.load({runId:'r',workspaceId:'w',kind:'tools'});
      // @ts-expect-error A window query requires its alias and node path.
      client.details.load({runId:'r',workspaceId:'w',kind:'context-window',query:{revision:'r'}});
      // @ts-expect-error An event page is not a tool.
      const wrong: Promise<ToolActivity> = client.details.load({runId:'r',workspaceId:'w',kind:'event-history',query:{after:1}});
      const follower: RunFollower = client.followRun({runId:'r',workspaceId:'w',receive: state => { const cursor: number = state.cursor; }});
      client.dispose();`);
    execFileSync(process.execPath, [require.resolve('typescript/bin/tsc'), '--noEmit', '--strict', '--target', 'ES2022', '--module', 'NodeNext', 'consumer.ts'], { cwd: directory, encoding: 'utf8' });
    const result = execFileSync(process.execPath, ['--input-type=module', '-e', `
      for (const name of ['window','document','EventSource','RTCPeerConnection']) Object.defineProperty(globalThis,name,{get(){throw new Error(name+' accessed')}});
      const {createClient} = await import('@zedflow/sdk');
      const client = createClient({baseUrl:'https://fixture.test/api',protocol:1,fetch:async()=>new Response('{"callId":"tool"}')});
      const value = await client.details.load({runId:'run',workspaceId:'ws',kind:'tools',id:'tool'});
      client.dispose(); console.log(value.callId);
    `], { cwd: directory, encoding: 'utf8' });
    assert.equal(result.trim(), 'tool');
  } finally { rmSync(directory, { recursive: true, force: true }); }
});

test('cache rejects non-JSON keys instead of colliding with a valid query or revision', () => {
  assert.throws(() => sdk.cacheKey(key({}, Number.NaN)), sdk.RequestValidationError);
  assert.throws(() => sdk.cacheKey(key({ after: Number.POSITIVE_INFINITY })), sdk.RequestValidationError);
  assert.equal(sdk.cacheKey(key({ after: undefined })), sdk.cacheKey(key()));
});
