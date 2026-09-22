import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as sdk from '../dist/index.js';

const bootstrap = (revision = 1) => ({ type: 'bootstrap', revision, cursor: revision, run: { id: 'run', workspaceId: 'ws', name: 'Fixture', status: 'running', composition: { id: 'flow', name: 'Flow', revision: 0, nodes: [], edges: [] }, state: { custom: ['kept'] }, messages: [], wait: null, timeline: [{ id: 'message', seq: 1, kind: 'message', role: 'assistant', text: 'Hello' }], toolActivities: [{ callId: 'tool', status: 'running' }] } });
const delta = (ops: unknown[], baseRevision = 1, revision = 2) => ({ type: 'delta', runId: 'run', workspaceId: 'ws', baseRevision, revision, cursor: revision, ops });
const tick = async () => { for (let i = 0; i < 8; i++) await Promise.resolve(); };
class Clock {
  time = 0;
  tasks = new Map<symbol, { at: number; fn: () => void }>();
  now = () => this.time;
  setTimeout = (fn: () => void, delay: number) => { const key = Symbol(); this.tasks.set(key, { at: this.time + delay, fn }); return () => { this.tasks.delete(key); }; };
  advance(ms: number) { this.time += ms; for (const [key, item] of [...this.tasks]) if (item.at <= this.time) { this.tasks.delete(key); item.fn(); } }
}

test('projection validates whole frames atomically and retains untouched conversation objects', () => {
  const projection = new sdk.RunProjection('run', 'ws');
  projection.apply(bootstrap());
  const previous = projection.state!;
  const malformed = delta([{ collection: 'toolActivities', id: 'tool', delete: true }, { collection: 'timeline', id: 'broken', value: { id: 'broken', kind: 'message', seq: 2, role: 'assistant', text: 12 } }]);
  assert.throws(() => projection.apply(malformed), sdk.ResponseValidationError);
  assert.equal(projection.state, previous);
  assert.throws(() => projection.apply(delta([{ collection: 'toolActivities', id: 'tool', delete: true }, { collection: 'toolActivities', id: 'wrong', value: { callId: 'other' } }])));
  assert.equal(projection.state, previous);
  projection.apply(delta([{ collection: 'toolActivities', id: 'tool', value: { callId: 'tool', status: 'completed' } }]));
  assert.equal(projection.state!.run.timeline, previous.run.timeline);
  assert.equal(projection.state!.run.composition, previous.run.composition);
  assert.equal(projection.state!.run.state, previous.run.state);
  assert.equal(projection.state!.run.toolActivities?.length, 1);
  assert.equal(projection.state!.run.toolActivities?.[0]?.status, 'completed');
  assert.deepEqual(projection.apply(delta([])), { changed: false, gap: false });
  assert.deepEqual(projection.apply(delta([], 3, 4)), { changed: false, gap: true });
  assert.throws(() => projection.apply({ ...delta([], 2, 3), workspaceId: 'other' }));
  assert.throws(() => projection.apply({ ...delta([], 2, 3), runId: 'other' }));
  assert.throws(() => projection.apply({ ...delta([], 2, 3), workspaceId: undefined }));
});

test('bootstrap, heartbeat gaps, memory bounds and scoped history are explicit', () => {
  const projection = new sdk.RunProjection('run', 'ws', undefined, { maxEntities: 2 });
  assert.deepEqual(projection.apply({ type: 'heartbeat', runId: 'run', workspaceId: 'ws', revision: 0, cursor: 0 }), { changed: false, gap: true });
  projection.apply(bootstrap());
  const previous = projection.state;
  assert.throws(() => projection.apply(delta([1, 2, 3].map(i => ({ collection: 'toolActivities', id: String(i), value: { callId: String(i) } })))));
  assert.equal(projection.state, previous);
  assert.throws(() => projection.prependTimeline({ runId: 'other', workspaceId: 'ws' }, { entries: [], before: null, hasMore: false }));
  projection.prependTimeline({ runId: 'run', workspaceId: 'ws' }, { entries: [{ id: 'older', seq: 0, kind: 'message', role: 'user', text: 'Old' }], before: 0, hasMore: false });
  assert.equal(projection.state!.run.timeline?.[1], previous!.run.timeline?.[0]);
});

test('sync recovers gaps by cursor, reconnects and ignores late responses after disposal', async () => {
  const clock = new Clock();
  const queries: sdk.SyncRequest[] = [];
  let observer: sdk.RunStreamObserver | undefined;
  let closed = 0;
  const published: sdk.RunProjectionState[] = [];
  const sync = sdk.followRun({ runId: 'run', workspaceId: 'ws', clock, pollIntervalMs: 100, requestTimeoutMs: 50, reconnectMs: 10,
    transport: { snapshot: async request => { queries.push(request); return queries.length === 1 ? bootstrap() : delta([], 1, 2); } },
    stream: { subscribe: (_request, next) => { observer = next; return () => { closed++; }; } },
    receive: value => published.push(value),
  });
  await tick();
  assert.equal(queries[0]?.after, undefined);
  observer!.frame({ type: 'heartbeat', runId: 'run', workspaceId: 'ws', revision: 2, cursor: 2 }, 'sse');
  await tick();
  assert.equal(queries[1]?.after, 1);
  assert.equal(sync.state?.revision, 2);
  observer!.error(new Error('disconnected'));
  clock.advance(10); await tick();
  assert.equal(closed, 1);
  sync.close();
  const count = published.length;
  observer!.frame(bootstrap(9), 'sse');
  clock.advance(1000); await tick();
  assert.equal(published.length, count);
  assert.equal(clock.tasks.size, 0);
});

test('current heartbeats and duplicate deltas do not refetch or republish, while a gap catches up', async () => {
  const clock = new Clock(), queries: sdk.SyncRequest[] = [], published: sdk.RunProjectionState[] = [];
  let observer!: sdk.RunStreamObserver;
  const next = delta([{ collection: 'timeline', id: 'next', value: { id: 'next', seq: 2, kind: 'message', role: 'assistant', text: 'Next' } }]);
  const catchup = delta([{ collection: 'timeline', id: 'recovered', value: { id: 'recovered', seq: 3, kind: 'message', role: 'assistant', text: 'Recovered' } }], 2, 3);
  const sync = sdk.followRun({ runId: 'run', workspaceId: 'ws', clock,
    transport: { snapshot: async request => { queries.push(request); return queries.length === 1 ? bootstrap() : catchup; } },
    stream: { subscribe: (_request, next) => { observer = next; return () => {}; } },
    receive: value => published.push(value),
  });
  try {
    await tick();
    assert.equal(queries.length, 1); assert.equal(queries[0]!.after, undefined);
    assert.equal(published.length, 1);
    const initial = sync.state;
    assert.equal(observer.frame({ type: 'heartbeat', runId: 'run', workspaceId: 'ws', revision: 1, cursor: 1 }, 'sse'), true);
    await tick();
    assert.equal(queries.length, 1); assert.equal(published.length, 1); assert.equal(sync.state, initial);
    assert.equal(observer.frame(next, 'sse'), true);
    await tick();
    assert.equal(queries.length, 1); assert.equal(published.length, 2);
    assert.equal(sync.state?.revision, 2); assert.equal(sync.state?.cursor, 2);
    assert.deepEqual(sync.state?.run.timeline?.map(entry => entry.text), ['Hello', 'Next']);
    const advanced = sync.state;
    assert.equal(observer.frame(next, 'sse'), true);
    clock.advance(1000); await tick();
    assert.equal(queries.length, 1); assert.equal(published.length, 2); assert.equal(sync.state, advanced);
    assert.equal(observer.frame({ type: 'heartbeat', runId: 'run', workspaceId: 'ws', revision: 3, cursor: 3 }, 'sse'), false);
    assert.equal(sync.state, advanced);
    await tick();
    assert.equal(queries.length, 2); assert.equal(queries[1]!.after, 2);
    assert.equal(published.length, 3); assert.equal(published[2], sync.state);
    assert.equal(sync.state?.revision, 3); assert.equal(sync.state?.cursor, 3);
    assert.deepEqual(sync.state?.run.timeline?.map(entry => entry.text), ['Hello', 'Next', 'Recovered']);
    assert.equal(sync.state?.run.timeline?.[0], initial?.run.timeline?.[0]);
  } finally { sync.close(); }
  assert.equal(clock.tasks.size, 0);
});

test('delta cursor regression and invalid initial state never publish a changed run', () => {
  const projection = new sdk.RunProjection('run', 'ws'); projection.apply(bootstrap());
  const state = projection.state;
  assert.throws(() => projection.apply({ ...delta([]), cursor: 0 }), sdk.SyncProtocolError);
  assert.throws(() => projection.apply(delta([{ collection: 'meta', value: { workspaceId: 'wrong' } }])), sdk.SyncProtocolError);
  assert.equal(projection.state, state);
  assert.throws(() => new sdk.RunProjection('other', 'ws', state), sdk.SyncProtocolError);
});

test('chunks are reordered, deduplicated, bounded in aggregate and expire on the injected clock', () => {
  const clock = new Clock();
  const decoder = new sdk.RunFrameDecoder(clock, { maxChars: 200, maxChunks: 3, maxAssemblies: 2, chunkTtlMs: 10 });
  const chunk = (id: string, index: number, data: string, total = 2) => JSON.stringify({ type: 'chunk', id, index, total, data });
  assert.equal(decoder.decode(chunk('one', 1, 'true}')), undefined);
  assert.equal(decoder.decode(chunk('one', 1, 'true}')), undefined);
  assert.equal(decoder.bufferedChars, 5);
  assert.deepEqual(decoder.decode(chunk('one', 0, '{"ok":')), { ok: true });
  assert.equal(decoder.bufferedChars, 0);
  decoder.decode(chunk('old', 0, 'unfinished'));
  assert.throws(() => decoder.decode(chunk('old', 0, 'different')), sdk.SyncProtocolError);
  assert.throws(() => decoder.decode(chunk('old', 1, 'x', 3)), sdk.SyncProtocolError);
  clock.advance(10); decoder.expire(); assert.equal(decoder.pending, 0);
  assert.throws(() => decoder.decode(chunk('bad', 2, 'x')), sdk.SyncProtocolError);
  assert.throws(() => decoder.decode('{'), sdk.JsonDecodeError);
  decoder.decode(chunk('one', 0, 'x'.repeat(100)));
  decoder.decode(chunk('two', 0, 'x'.repeat(100)));
  assert.equal(decoder.bufferedChars, 200);
  assert.throws(() => decoder.decode(chunk('three', 0, 'x')), sdk.SyncProtocolError);
  assert.throws(() => decoder.decode(chunk('two', 1, 'x')), sdk.SyncProtocolError);
  decoder.clear(); assert.equal(decoder.pending, 0); assert.equal(decoder.bufferedChars, 0);
});

test('timeout unblocks a noncompliant transport and late HTTP response cannot replace a newer projection', async () => {
  const clock = new Clock();
  let resolve!: (value: unknown) => void;
  let calls = 0, observer: sdk.RunStreamObserver | undefined;
  const sync = sdk.followRun({ runId: 'run', workspaceId: 'ws', clock, requestTimeoutMs: 10, pollIntervalMs: 20,
    transport: { snapshot: async () => { calls++; if (calls === 1) return new Promise(yes => { resolve = yes; }); return bootstrap(2); } },
    stream: { subscribe: (_request, next) => { observer = next; return () => {}; } }, receive: () => {},
  });
  clock.advance(10); await tick();
  await sync.refresh(); assert.equal(sync.state?.revision, 2);
  resolve(bootstrap(9)); await tick(); assert.equal(sync.state?.revision, 2);
  observer!.frame(bootstrap(3), 'sse'); assert.equal(sync.state?.revision, 3);
  sync.close(); assert.equal(clock.tasks.size, 0);
});

test('invalid stream frame triggers repair, wrong-run responses never publish, lifecycle wakes and unsubscribes', async () => {
  const clock = new Clock();
  let online = true, visible = true, wake = () => {}, subscriptions = 0, calls = 0;
  let observer: sdk.RunStreamObserver | undefined;
  const published: number[] = [];
  const sync = sdk.followRun({ runId: 'run', workspaceId: 'ws', clock, pollIntervalMs: 20,
    connectivity: { online: () => online, visible: () => visible, subscribe: listener => { wake = listener; subscriptions++; return () => { subscriptions--; }; } },
    transport: { snapshot: async () => { calls++; return calls === 2 ? { ...bootstrap(9), run: { ...bootstrap(9).run, id: 'other' } } : bootstrap(calls); } },
    stream: { subscribe: (_request, next) => { observer = next; return () => {}; } }, receive: value => published.push(value.revision),
  });
  await tick(); observer!.frame({ type: 'delta', broken: true }, 'sse'); await tick();
  assert.deepEqual(published, [1]);
  online = false; wake(); clock.advance(50); await tick(); assert.equal(calls, 2);
  online = true; visible = false; wake(); assert.equal(calls, 2);
  visible = true; wake(); await tick(); assert.equal(sync.state?.revision, 3);
  sync.close(); wake(); assert.equal(subscriptions, 0); assert.equal(clock.tasks.size, 0);
});

test('daemon connection deduplicates, expires at time zero, and releases late requests/timers', async () => {
  const clock = new Clock();
  let calls = 0, resolve!: (value: sdk.DaemonHealth) => void;
  let subscribed = 0;
  const connection = sdk.createDaemonConnection({ clock, intervalMs: 100, timeoutMs: 20, staleMs: 10,
    connectivity: { online: () => true, visible: () => true, subscribe: () => { subscribed++; return () => { subscribed--; }; } },
    daemon: { health: async () => { calls++; if (calls === 1) return { daemon: { id: 'id', host: 'fixture' } }; return new Promise(yes => { resolve = yes; }); } },
  });
  await tick(); assert.equal(connection.state.connected, true); assert.equal(connection.state.lastSeen, 0);
  clock.advance(10); assert.equal(connection.state.connected, false);
  const request = connection.check(); assert.equal(connection.check(), request);
  const aborted = assert.rejects(request, sdk.RequestAbortedError);
  connection.dispose(); await aborted;
  resolve({ daemon: { id: 'late', host: 'late' } }); await tick();
  assert.equal(connection.state.hostname, 'fixture'); assert.equal(connection.state.connected, false);
  assert.equal(clock.tasks.size, 0); assert.equal(subscribed, 0);
});

test('catchup drains partial pages to the advertised head and recovers missing bases with bootstrap', async () => {
  const clock = new Clock(), queries: sdk.SyncRequest[] = [];
  let observer: sdk.RunStreamObserver | undefined;
  const sync = sdk.followRun({ runId: 'run', workspaceId: 'ws', clock,
    transport: { snapshot: async request => { queries.push(request); return queries.length === 1 ? bootstrap() : queries.length === 2 ? delta([], 1, 2) : queries.length === 3 ? delta([], 2, 3) : queries.length === 4 ? delta([], 7, 8) : bootstrap(8); } },
    stream: { subscribe: (_request, next) => { observer = next; return () => {}; } }, receive: () => {},
  });
  await tick();
  observer!.frame({ type: 'heartbeat', runId: 'run', workspaceId: 'ws', revision: 3, cursor: 3 }, 'sse');
  await tick(); clock.advance(0); await tick();
  assert.equal(sync.state?.revision, 3);
  assert.deepEqual(queries.map(request => request.after), [undefined, 1, 2]);
  observer!.frame(delta([], 7, 8), 'sse'); await tick(); clock.advance(0); await tick();
  assert.equal(queries.at(-1)?.after, undefined);
  assert.equal(sync.state?.revision, 8);
  sync.close();
});

class FakeEventSource extends EventTarget {
  onerror: ((event: Event) => void) | null = null;
  closed = false;
  close() { this.closed = true; }
  send(value: unknown) { this.dispatchEvent(new MessageEvent('sync', { data: JSON.stringify(value) })); }
}
class FakeDataChannel {
  onmessage: ((event: MessageEvent) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  closed = false;
  close() { this.closed = true; }
}
class FakePeer extends EventTarget {
  channel = new FakeDataChannel();
  connectionState = 'connected'; iceGatheringState = 'complete';
  localDescription: RTCSessionDescriptionInit | null = null;
  onconnectionstatechange: (() => void) | null = null;
  closed = false; remote = 0;
  createDataChannel() { return this.channel; }
  async createOffer() { return { type: 'offer', sdp: 'offer' }; }
  async setLocalDescription(value: RTCSessionDescriptionInit) { this.localDescription = value; }
  async setRemoteDescription() { this.remote++; }
  close() { this.closed = true; }
}

test('browser adapter promotes only accepted RTC, falls back on silence and cleans all resources', async () => {
  const clock = new Clock(), sources: FakeEventSource[] = [], urls: string[] = [];
  const peer = new FakePeer(), controller = new AbortController();
  const client = sdk.createClient({ baseUrl: 'https://fixture.test/nested/api', protocol: 3, fetch: async url => new Response(JSON.stringify(url.endsWith('/rtc/config') ? { iceServers: [], iceTransportPolicy: 'all' } : { type: 'answer', sdp: 'answer' })) });
  const stream = sdk.createBrowserRunTransport({ baseUrl: 'https://fixture.test/nested/api', daemon: client.daemon, runs: client.runs, clock, rtcTimeoutMs: 50,
    eventSource: url => { urls.push(url); const source = new FakeEventSource(); sources.push(source); return source as unknown as EventSource; },
    peerConnection: () => peer as unknown as RTCPeerConnection,
  });
  let accept = false, received = 0;
  const close = stream.subscribe({ runId: 'run/with slash', workspaceId: 'ws/with slash', after: 2, signal: controller.signal }, { frame: () => { received++; return accept; }, error: error => { throw error; } });
  await tick();
  assert.equal(new URL(urls[0]!).pathname, '/nested/api/runs/run%2Fwith%20slash/events');
  assert.equal(new URL(urls[0]!).searchParams.get('workspaceId'), 'ws/with slash');
  await new Promise<void>(resolve => setImmediate(resolve));
  assert.equal(peer.remote, 1);
  peer.channel.onmessage!(new MessageEvent('message', { data: JSON.stringify(bootstrap(3)) }));
  assert.equal(sources[0]!.closed, false);
  accept = true;
  peer.channel.onmessage!(new MessageEvent('message', { data: JSON.stringify(bootstrap(4)) }));
  assert.equal(sources[0]!.closed, true);
  clock.advance(50); await tick();
  assert.equal(peer.closed, true); assert.equal(sources.length, 2);
  assert.equal(new URL(urls[1]!).searchParams.get('after'), '4');
  controller.abort(); close();
  sources[1]!.send(bootstrap(9)); assert.equal(received, 2);
  assert.equal(sources[1]!.closed, true); assert.equal(clock.tasks.size, 0);
  client.dispose();
});

test('closing with pending I/O immediately releases timers and observes a late failure', async () => {
  const clock = new Clock();
  let reject!: (error: unknown) => void;
  const sync = sdk.followRun({ runId: 'run', workspaceId: 'ws', clock, transport: { snapshot: () => new Promise((_yes, no) => { reject = no; }) }, receive: () => { throw new Error('No frame should publish'); } });
  sync.close();
  assert.equal(clock.tasks.size, 0);
  reject(new Error('Late socket failure')); await tick();
  assert.equal(sync.state, undefined);
});

test('sync metrics count duplicates and distinguish stream wire measurements from HTTP frames', async () => {
  const clock = new Clock(), reports: sdk.LiveStatus[] = [];
  let observer: sdk.RunStreamObserver | undefined;
  const sync = sdk.followRun({ runId: 'run', workspaceId: 'ws', clock, transport: { snapshot: async () => bootstrap() },
    stream: { subscribe: (_request, next) => { observer = next; return () => {}; } }, receive: () => {}, status: value => reports.push(value),
  });
  await tick(); observer!.traffic!({ receivedChars: 42, decodeMs: 1 }); observer!.frame(bootstrap(), 'sse');
  const metrics = reports.at(-1)!.metrics!;
  assert.equal(metrics.frames, 2); assert.equal(metrics.duplicates, 1);
  assert.equal(metrics.receivedChars, 42); assert.equal(metrics.decodeMs, 1); assert.equal(metrics.wireMeasurementScope, 'stream');
  sync.close();
});
