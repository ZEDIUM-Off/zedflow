import { JsonDecodeError } from '../core/errors.js';
import { validateResponse } from '../core/schema.js';
import type { DaemonClient } from '../daemon/client.js';
import type { RunsClient } from '../runs/client.js';
import { rtcChunkSchema } from '../runs/results.js';
import { SyncProtocolError } from '../runs/projection.js';
import { systemClock, type Clock, type Connectivity, type RunStreamTransport } from './http.js';

export interface FrameLimits { maxChars?: number; maxAssemblies?: number; maxChunks?: number; chunkTtlMs?: number }
interface Assembly { parts: Map<number, string>; total: number; chars: number; expires: number }
/** Bounds are global across incomplete messages, not multiplied by the number of assemblies. */
export class RunFrameDecoder {
  private readonly assemblies = new Map<string, Assembly>();
  private chars = 0;
  private readonly maxChars: number;
  private readonly maxAssemblies: number;
  private readonly maxChunks: number;
  private readonly ttl: number;
  constructor(private readonly clock: Pick<Clock, 'now'> = systemClock, limits: FrameLimits = {}) {
    this.maxChars = limits.maxChars ?? 32 * 1024 * 1024;
    this.maxAssemblies = limits.maxAssemblies ?? 8;
    this.maxChunks = limits.maxChunks ?? 8192;
    this.ttl = limits.chunkTtlMs ?? 12_000;
    for (const value of [this.maxChars, this.maxAssemblies, this.maxChunks, this.ttl]) if (!Number.isSafeInteger(value) || value < 1) throw new RangeError('Frame limits must be positive integers');
  }
  get bufferedChars(): number { return this.chars; }
  get pending(): number { return this.assemblies.size; }
  clear(): void { this.assemblies.clear(); this.chars = 0; }
  expire(): void { for (const [id, value] of this.assemblies) if (value.expires <= this.clock.now()) { this.chars -= value.chars; this.assemblies.delete(id); } }
  private parse(raw: string): unknown { try { return JSON.parse(raw); } catch (error) { throw new JsonDecodeError('runs.stream', error); } }
  decode(raw: unknown): unknown {
    this.expire();
    if (typeof raw !== 'string' || raw.length > this.maxChars) throw new SyncProtocolError('Invalid or oversized stream message');
    const value = this.parse(raw);
    if (typeof value !== 'object' || value === null || !('type' in value) || value.type !== 'chunk') return value;
    const chunk = validateResponse('runs.chunk', rtcChunkSchema, value);
    if (chunk.total > this.maxChunks || chunk.index >= chunk.total) throw new SyncProtocolError('Invalid chunk index/count');
    let assembly = this.assemblies.get(chunk.id);
    if (!assembly) {
      if (this.assemblies.size >= this.maxAssemblies) throw new SyncProtocolError('Too many incomplete messages');
      assembly = { parts: new Map(), total: chunk.total, chars: 0, expires: this.clock.now() + this.ttl };
      this.assemblies.set(chunk.id, assembly);
    }
    if (assembly.total !== chunk.total) throw new SyncProtocolError('Conflicting chunk count');
    const old = assembly.parts.get(chunk.index);
    if (old !== undefined && old !== chunk.data) throw new SyncProtocolError('Conflicting duplicate chunk');
    if (old === undefined) {
      if (this.chars + chunk.data.length > this.maxChars) throw new SyncProtocolError('Chunk memory limit exceeded');
      assembly.parts.set(chunk.index, chunk.data); assembly.chars += chunk.data.length; this.chars += chunk.data.length;
    }
    if (assembly.parts.size !== assembly.total) return undefined;
    this.assemblies.delete(chunk.id); this.chars -= assembly.chars;
    return this.parse(Array.from({ length: assembly.total }, (_, index) => assembly.parts.get(index)!).join(''));
  }
}

/** Explicit browser lifecycle injection; calling this function is the first DOM access. */
export function createBrowserConnectivity(targetWindow: Window = window, targetDocument: Document = document): Connectivity {
  return {
    visible: () => targetDocument.visibilityState === 'visible', online: () => targetWindow.navigator.onLine,
    subscribe: wake => {
      targetWindow.addEventListener('online', wake); targetWindow.addEventListener('offline', wake); targetDocument.addEventListener('visibilitychange', wake);
      return () => { targetWindow.removeEventListener('online', wake); targetWindow.removeEventListener('offline', wake); targetDocument.removeEventListener('visibilitychange', wake); };
    },
  };
}
export interface BrowserRunTransportOptions {
  baseUrl: string;
  runs: RunsClient;
  daemon: DaemonClient;
  clock?: Clock;
  limits?: FrameLimits;
  /** Native EventSource authenticates with cookies; inject a factory for other stream credentials. */
  eventSource?: (url: string) => EventSource;
  peerConnection?: (config: RTCConfiguration) => RTCPeerConnection;
  rtc?: boolean;
  rtcTimeoutMs?: number;
}
/** SSE begins immediately. RTC is promoted only after a frame is accepted by the follower. */
export function createBrowserRunTransport(options: BrowserRunTransportOptions): RunStreamTransport {
  const base = new URL(`${options.baseUrl.replace(/\/+$/, '')}/`), clock = options.clock ?? systemClock;
  if (!['http:', 'https:'].includes(base.protocol) || base.search || base.hash || base.username || base.password) throw new TypeError('An absolute HTTP API base URL is required');
  const timeout = options.rtcTimeoutMs ?? 12_000;
  if (!Number.isFinite(timeout) || timeout <= 0) throw new RangeError('RTC timeout must be positive');
  return { subscribe(request, observer) {
    const decoder = new RunFrameDecoder(clock, options.limits), negotiation = new AbortController();
    let closed = false, source: EventSource | undefined, peer: RTCPeerConnection | undefined, rtcStopped = options.rtc === false;
    let channel: RTCDataChannel | undefined, lastRtc = clock.now(), cancelWatch: (() => void) | undefined;
    let cursor = request.after;
    function decode(raw: unknown): unknown {
      const started = clock.now();
      try { return decoder.decode(raw); }
      finally { observer.traffic?.({ receivedChars: typeof raw === 'string' ? raw.length : 0, decodeMs: clock.now() - started }); }
    }
    const clearSource = () => { const old = source; source = undefined; if (old) { old.onerror = null; old.close(); } };
    function startSse(): void {
      if (closed || source) return;
      const url = new URL(`runs/${encodeURIComponent(request.runId)}/events`, base);
      url.searchParams.set('workspaceId', request.workspaceId);
      if (cursor !== undefined) url.searchParams.set('after', String(cursor));
      const stream = (options.eventSource ?? (url => new EventSource(url)))(url.href); source = stream;
      stream.addEventListener('sync', event => {
        if (closed || source !== stream) return;
        try { const value = decode((event as MessageEvent<unknown>).data); if (value !== undefined && observer.frame(value, 'sse') && typeof value === 'object' && value !== null && 'cursor' in value && typeof value.cursor === 'number') cursor = value.cursor; }
        catch (error) { observer.error(error); }
      });
      stream.onerror = event => { if (!closed && source === stream) observer.error(event); };
    }
    function stopRtc(): void {
      rtcStopped = true; negotiation.abort(); cancelWatch?.(); decoder.clear();
      if (channel) { channel.onmessage = null; channel.onerror = null; channel.onclose = null; channel.close(); channel = undefined; }
      if (peer) { peer.onconnectionstatechange = null; peer.close(); peer = undefined; }
      if (!closed) { try { startSse(); } catch (error) { observer.error(error); } }
    }
    function close(): void {
      if (closed) return; closed = true; request.signal.removeEventListener('abort', close); clearSource(); stopRtc();
    }
    request.signal.addEventListener('abort', close, { once: true });
    if (request.signal.aborted) { close(); return close; }
    try { startSse(); } catch (error) { observer.error(error); }
    function watchRtc(): void {
      if (closed || rtcStopped) return;
      decoder.expire();
      if (clock.now() - lastRtc >= timeout) { stopRtc(); return; }
      cancelWatch = clock.setTimeout(watchRtc, Math.min(timeout, 1000));
    }
    async function connectRtc(): Promise<void> {
      try {
        const config = await options.daemon.rtcConfig(negotiation.signal);
        if (closed || rtcStopped) return;
        const pc = (options.peerConnection ?? (config => new RTCPeerConnection(config)))(config); peer = pc;
        const data = pc.createDataChannel('zedflow-events', { ordered: true }); channel = data;
        data.onclose = stopRtc; data.onerror = stopRtc;
        data.onmessage = event => {
          if (closed || rtcStopped) return;
          try {
            const value = decode(event.data);
            if (value === undefined) return;
            const accepted = observer.frame(value, 'webrtc');
            if (closed || rtcStopped || !accepted) return;
            // The observer synchronously validates the complete frame and closes on failure/gap.
            if (typeof value === 'object' && value !== null && 'cursor' in value && typeof value.cursor === 'number') cursor = value.cursor;
            lastRtc = clock.now(); clearSource();
          } catch { stopRtc(); }
        };
        pc.onconnectionstatechange = () => { if (['failed', 'disconnected', 'closed'].includes(pc.connectionState)) stopRtc(); };
        await pc.setLocalDescription(await pc.createOffer());
        if (closed || rtcStopped) return;
        if (pc.iceGatheringState !== 'complete') await new Promise<void>((resolve, reject) => {
          const cleanup = () => { pc.removeEventListener('icegatheringstatechange', check); negotiation.signal.removeEventListener('abort', abort); cancel(); };
          const check = () => { if (pc.iceGatheringState === 'complete') { cleanup(); resolve(); } };
          const abort = () => { cleanup(); reject(new Error('RTC negotiation aborted')); };
          const cancel = clock.setTimeout(() => { cleanup(); reject(new Error('RTC gathering timed out')); }, Math.min(timeout, 6000));
          pc.addEventListener('icegatheringstatechange', check); negotiation.signal.addEventListener('abort', abort, { once: true });
          if (negotiation.signal.aborted) abort(); else check();
        });
        if (closed || rtcStopped) return;
        const sdp = pc.localDescription?.sdp;
        if (!sdp) throw new SyncProtocolError('Missing local RTC offer');
        const answer = await options.runs.offerRtc(request.runId, { type: 'offer', sdp, after: cursor ?? 0 }, { workspaceId: request.workspaceId }, negotiation.signal);
        if (!closed && !rtcStopped) await pc.setRemoteDescription(answer);
      } catch { if (!closed) stopRtc(); }
    }
    if (!rtcStopped && !closed) { watchRtc(); void connectRtc(); }
    return close;
  } };
}
