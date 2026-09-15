import { abortable } from '../core/cache.js';
import { RequestAbortedError } from '../core/errors.js';
import type { DaemonClient } from './client.js';
import type { DaemonHealth } from './model.js';
import { alwaysConnected, systemClock, type Clock, type Connectivity, type Cancel } from '../transports/http.js';

export interface DaemonConnectionState { health: DaemonHealth | undefined; lastSeen: number | undefined; connected: boolean; hostname: string; error: unknown }
export interface DaemonConnectionOptions {
  daemon: Pick<DaemonClient, 'health'>; clock?: Clock; connectivity?: Connectivity;
  intervalMs?: number; timeoutMs?: number; staleMs?: number; receive?(state: DaemonConnectionState): void;
}
export interface DaemonConnection { readonly state: DaemonConnectionState; seen(): void; check(): Promise<DaemonHealth>; dispose(): void }
/** Starts explicitly; health/seen are per instance, and late responses cannot resurrect a disposed connection. */
export function createDaemonConnection(options: DaemonConnectionOptions): DaemonConnection {
  const clock = options.clock ?? systemClock, connectivity = options.connectivity ?? alwaysConnected;
  const interval = options.intervalMs ?? 5000, timeout = options.timeoutMs ?? 4000, stale = options.staleMs ?? 12000;
  for (const value of [interval, timeout, stale]) if (!Number.isFinite(value) || value <= 0) throw new RangeError('Connection delays must be positive');
  let health: DaemonHealth | undefined, lastSeen: number | undefined, error: unknown, pending: Promise<DaemonHealth> | undefined;
  let closed = false, controller: AbortController | undefined, cancelTimer: Cancel | undefined, cancelStale: Cancel | undefined;
  let cancelRequestTimeout: Cancel | undefined;
  const state = (): DaemonConnectionState => ({ health, lastSeen, connected: !closed && connectivity.online() && lastSeen !== undefined && clock.now() - lastSeen < stale, hostname: health?.daemon?.host ?? health?.workspace?.host ?? 'Daemon', error });
  const publish = () => { if (!closed) options.receive?.(state()); };
  const seen = () => { if (closed) return; lastSeen = clock.now(); error = undefined; cancelStale?.(); cancelStale = clock.setTimeout(publish, stale); publish(); };
  function check(): Promise<DaemonHealth> {
    if (closed) return Promise.reject(new RequestAbortedError('daemon.health', 'Connection disposed'));
    if (pending) return pending;
    const request = new AbortController(); controller = request;
    const cancelTimeout = clock.setTimeout(() => request.abort('Health timed out'), timeout);
    cancelRequestTimeout = cancelTimeout;
    pending = abortable('daemon.health', request.signal, () => options.daemon.health(request.signal))
      .then(value => { if (!closed && !request.signal.aborted) { health = value; seen(); } return value; })
      .catch(cause => { if (!closed) { error = cause; publish(); } throw cause; })
      .finally(() => { cancelTimeout(); if (controller === request) { controller = undefined; cancelRequestTimeout = undefined; } pending = undefined; });
    return pending;
  }
  const wake = () => { if (closed) return; if (connectivity.online() && connectivity.visible()) void check().catch(() => {}); else { if (!connectivity.online()) controller?.abort('Offline'); publish(); } };
  const schedule = () => { cancelTimer = clock.setTimeout(() => { wake(); if (!closed) schedule(); }, interval); };
  const unsubscribe = connectivity.subscribe(wake);
  wake(); schedule();
  return { get state() { return state(); }, seen, check, dispose: () => { if (closed) return; closed = true; cancelTimer?.(); cancelStale?.(); cancelRequestTimeout?.(); controller?.abort('Connection disposed'); unsubscribe(); } };
}
