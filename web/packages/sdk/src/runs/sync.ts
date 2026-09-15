import { abortable } from '../core/cache.js';
import { RunProjection, type RunProjectionState, type RunScope, type ProjectionLimits } from './projection.js';
import { alwaysConnected, systemClock, type Clock, type Connectivity, type Cancel, type RunStreamTransport, type RunSyncTransport, type StreamKind } from '../transports/http.js';

export interface LiveMetrics {
  frames: number; bootstraps: number; deltas: number; heartbeats: number; duplicates: number;
  gaps: number; invalidFrames: number; applyMs: number; publicationMs: number;
  /** Wire measurements cover the injected stream only; HTTP does not expose encoded body measurements. */
  receivedChars: number; decodeMs: number; wireMeasurementScope: 'stream';
}
export interface LiveStatus { transport: 'connecting' | StreamKind | 'http' | 'offline'; error?: unknown; metrics?: LiveMetrics }
export interface FollowRunOptions extends RunScope {
  transport: RunSyncTransport; stream?: RunStreamTransport; clock?: Clock; connectivity?: Connectivity;
  initial?: RunProjectionState; limits?: ProjectionLimits; receive(state: RunProjectionState): void;
  status?(status: LiveStatus): void; seen?(): void;
  pollIntervalMs?: number; requestTimeoutMs?: number; reconnectMs?: number;
}
export interface RunFollower { readonly state: RunProjectionState | undefined; refresh(): Promise<void>; close(): void; prependTimeline(scope: RunScope, page: unknown): void }

export function followRun(options: FollowRunOptions): RunFollower {
  const clock = options.clock ?? systemClock, connectivity = options.connectivity ?? alwaysConnected;
  const projection = new RunProjection(options.runId, options.workspaceId, options.initial, options.limits);
  const interval = options.pollIntervalMs ?? 1000, timeoutMs = options.requestTimeoutMs ?? 4000, reconnectMs = options.reconnectMs ?? 1000;
  for (const value of [interval, timeoutMs, reconnectMs]) if (!Number.isFinite(value) || value <= 0) throw new RangeError('Sync delays must be positive');
  let closed = false, pending: Promise<void> | undefined, controller: AbortController | undefined;
  let needsBootstrap = !projection.state, needsCatchup = false;
  let cancelTimer: Cancel | undefined, cancelStream: Cancel | undefined, cancelReconnect: Cancel | undefined;
  let targetRevision = 0;
  let lastStream: number | undefined;
  let cancelRequestTimeout: Cancel | undefined;
  let streamController: AbortController | undefined, streamGeneration = 0;
  const metrics: LiveMetrics = { frames: 0, bootstraps: 0, deltas: 0, heartbeats: 0, duplicates: 0, gaps: 0, invalidFrames: 0, applyMs: 0, publicationMs: 0, receivedChars: 0, decodeMs: 0, wireMeasurementScope: 'stream' };
  const scope = { runId: options.runId, workspaceId: options.workspaceId };
  function report(status: LiveStatus): void { if (!closed) options.status?.({ ...status, metrics: { ...metrics } }); }
  function accept(value: unknown, transport: StreamKind | 'http'): boolean {
    if (closed) return false;
    const started = clock.now();
    let result: ReturnType<RunProjection['apply']>;
    try { result = projection.apply(value); } catch (error) { metrics.invalidFrames++; throw error; }
    finally { metrics.applyMs += clock.now() - started; }
    metrics.frames++;
    if (typeof value === 'object' && value !== null && 'type' in value) {
      if (value.type === 'bootstrap') metrics.bootstraps++;
      else if (value.type === 'delta') metrics.deltas++;
      else metrics.heartbeats++;
      if (!result.changed && !result.gap && value.type !== 'heartbeat') metrics.duplicates++;
    }
    if (typeof value === 'object' && value !== null && 'revision' in value && typeof value.revision === 'number') targetRevision = Math.max(targetRevision, value.revision);
    if (result.gap) { metrics.gaps++; needsCatchup = true; if (!projection.state) needsBootstrap = true; void refresh(); return false; }
    needsBootstrap = false;
    needsCatchup = (projection.state?.revision ?? 0) < targetRevision;
    options.seen?.();
    if (transport !== 'http') lastStream = clock.now();
    if (result.changed && projection.state) {
      const started = clock.now();
      try { options.receive(projection.state); } finally { metrics.publicationMs += clock.now() - started; }
    }
    if (transport !== 'http' || lastStream === undefined || clock.now() - lastStream >= 12000) report({ transport });
    return true;
  }
  function stopStream(): void { lastStream = undefined; streamGeneration++; streamController?.abort(); streamController = undefined; cancelStream?.(); cancelStream = undefined; }
  function reconnect(error: unknown): void {
    if (closed) return;
    stopStream(); report({ transport: 'offline', error });
    needsCatchup = true; void refresh();
    if (!cancelReconnect) cancelReconnect = clock.setTimeout(() => { cancelReconnect = undefined; startStream(); }, reconnectMs);
  }
  function startStream(): void {
    if (closed || !options.stream || !connectivity.online() || cancelStream || cancelReconnect) return;
    const generation = ++streamGeneration;
    streamController = new AbortController();
    try {
      const cancel = options.stream.subscribe({ ...scope, ...(projection.state ? { after: projection.state.cursor } : {}), signal: streamController.signal }, {
        frame: (value, transport) => { if (closed || generation !== streamGeneration) return false; try { return accept(value, transport); } catch (error) { reconnect(error); return false; } },
        error: error => { if (!closed && generation === streamGeneration) reconnect(error); },
        traffic: value => { if (!closed && generation === streamGeneration) { metrics.receivedChars += value.receivedChars; metrics.decodeMs += value.decodeMs; } },
      });
      if (closed || generation !== streamGeneration) cancel(); else cancelStream = cancel;
    } catch (error) { reconnect(error); }
  }
  function refresh(): Promise<void> {
    if (closed || !connectivity.online()) return Promise.resolve();
    if (pending) return pending;
    needsCatchup = false;
    const request = new AbortController(); controller = request;
    const cancelTimeout = clock.setTimeout(() => request.abort('Snapshot timed out'), timeoutMs);
    cancelRequestTimeout = cancelTimeout;
    const after = needsBootstrap ? undefined : projection.state?.cursor;
    pending = abortable('runs.sync', request.signal, () => options.transport.snapshot({ ...scope, ...(after !== undefined ? { after } : {}), signal: request.signal }))
      .then(value => {
        if (closed || request.signal.aborted) return;
        const before = projection.state?.cursor;
        accept(value, 'http');
        // A stale/discontinuous HTTP page cannot repair its own base; request a bootstrap next.
        if (needsCatchup && before === projection.state?.cursor) needsBootstrap = true;
      }).catch(error => { if (!closed) report({ transport: 'offline', error }); })
      .finally(() => {
        cancelTimeout(); if (controller === request) { controller = undefined; cancelRequestTimeout = undefined; } pending = undefined;
        if (!closed && needsCatchup) schedule(after === undefined ? interval : 0);
      });
    return pending;
  }
  function schedule(delay = interval): void {
    cancelTimer?.();
    if (!closed) cancelTimer = clock.setTimeout(() => { cancelTimer = undefined; if (connectivity.visible() && connectivity.online()) { startStream(); if (lastStream === undefined || clock.now() - lastStream >= 12000 || needsCatchup) void refresh(); } schedule(); }, delay);
  }
  const unsubscribe = connectivity.subscribe(() => {
    if (closed) return;
    if (!connectivity.online()) { stopStream(); controller?.abort('Offline'); report({ transport: 'offline' }); }
    else if (connectivity.visible()) { startStream(); void refresh(); }
  });
  report({ transport: 'connecting' }); startStream(); void refresh(); schedule();
  return {
    get state() { return projection.state; }, refresh,
    prependTimeline: (pageScope, page) => { if (closed) return; projection.prependTimeline(pageScope, page); if (projection.state) options.receive(projection.state); },
    close: () => { if (closed) return; closed = true; cancelTimer?.(); cancelReconnect?.(); cancelRequestTimeout?.(); stopStream(); controller?.abort('Follower closed'); unsubscribe(); },
  };
}
