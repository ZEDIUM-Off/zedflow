import { jsonObjectSchema, type JsonValue } from './json.js';
import { validateInput } from './schema.js';
import { RequestAbortedError, RequestValidationError } from './errors.js';

export interface CacheIdentity {
  workspaceId: string;
  runId: string;
  kind: string;
  id?: string;
  revision?: string | number;
  query?: Readonly<Record<string, JsonValue | undefined>>;
}
export interface CacheEntry<T> { readonly loading: boolean; readonly value?: T; readonly error?: unknown }
export interface CacheOptions { capacity?: number }
interface Slot<T> { entry: CacheEntry<T>; controller: AbortController; promise: Promise<T> }

function canonical(value: JsonValue): JsonValue {
  if (Array.isArray(value)) return value.map(canonical);
  if (value !== null && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key]!)]));
  return value;
}
/** Same structured identity for lookup and load; undefined query values do not go on the wire. */
export function cacheKey(identity: CacheIdentity): string {
  if ([identity.workspaceId, identity.runId, identity.kind].some(value => typeof value !== 'string' || !value) || identity.id !== undefined && typeof identity.id !== 'string' || identity.revision !== undefined && !(typeof identity.revision === 'string' || typeof identity.revision === 'number' && Number.isFinite(identity.revision))) throw new RequestValidationError('cache.key', new Error('Invalid cache identity'));
  const query = validateInput('cache.key', jsonObjectSchema, Object.fromEntries(Object.entries(identity.query ?? {}).filter(([, value]) => value !== undefined)));
  return JSON.stringify([identity.workspaceId, identity.runId, identity.kind, identity.id ?? '', identity.revision ?? null, canonical(query)]);
}

/** Cancels even a transport that ignores AbortSignal, and observes its eventual rejection. */
export function abortable<T>(operation: string, signal: AbortSignal, load: () => Promise<T>): Promise<T> {
  if (signal.aborted) return Promise.reject(new RequestAbortedError(operation, signal.reason));
  return new Promise<T>((resolve, reject) => {
    const abort = () => { signal.removeEventListener('abort', abort); reject(new RequestAbortedError(operation, signal.reason)); };
    signal.addEventListener('abort', abort, { once: true });
    let promise: Promise<T>;
    try { promise = load(); } catch (error) { signal.removeEventListener('abort', abort); reject(error); return; }
    promise.then(value => { signal.removeEventListener('abort', abort); if (signal.aborted) abort(); else resolve(value); },
      error => { signal.removeEventListener('abort', abort); if (signal.aborted) abort(); else reject(error); });
  });
}

/** LRU includes pending entries: the bound never grows under a burst of distinct requests. */
export class DetailCache<T> {
  private readonly slots = new Map<string, Slot<T>>();
  private readonly listeners = new Set<() => void>();
  private readonly capacity: number;
  private closed = false;
  constructor(options: CacheOptions = {}) {
    this.capacity = options.capacity ?? 256;
    if (!Number.isSafeInteger(this.capacity) || this.capacity < 1) throw new RangeError('Cache capacity must be a positive integer');
  }
  get size(): number { return this.slots.size; }
  subscribe(listener: () => void): () => void { if (this.closed) return () => {}; this.listeners.add(listener); return () => { this.listeners.delete(listener); }; }
  private publish(): void { for (const listener of this.listeners) listener(); }
  entry(identity: CacheIdentity): CacheEntry<T> | undefined {
    const key = cacheKey(identity), slot = this.slots.get(key);
    if (slot) { this.slots.delete(key); this.slots.set(key, slot); }
    return slot?.entry;
  }
  load(identity: CacheIdentity, loader: (signal: AbortSignal) => Promise<T>): Promise<T> {
    if (this.closed) return Promise.reject(new RequestAbortedError('cache.load', 'Cache disposed'));
    const key = cacheKey(identity), existing = this.slots.get(key);
    if (existing && !('error' in existing.entry)) { this.slots.delete(key); this.slots.set(key, existing); return existing.promise; }
    if (existing) this.slots.delete(key);
    while (this.slots.size >= this.capacity) this.remove(this.slots.keys().next().value!);
    const controller = new AbortController();
    const promise = abortable('cache.load', controller.signal, () => loader(controller.signal)).then(value => {
      if (this.slots.get(key) === slot && !controller.signal.aborted) { slot.entry = { loading: false, value }; this.publish(); }
      return value;
    }, error => {
      if (this.slots.get(key) === slot && !controller.signal.aborted) { slot.entry = { loading: false, error }; this.publish(); }
      throw error;
    });
    const slot: Slot<T> = { controller, promise, entry: { loading: true } };
    this.slots.set(key, slot); this.publish();
    return promise;
  }
  private remove(key: string): void { const slot = this.slots.get(key); this.slots.delete(key); slot?.controller.abort('Cache entry removed'); }
  invalidate(identity: CacheIdentity): void { this.remove(cacheKey(identity)); this.publish(); }
  clear(): void { for (const key of this.slots.keys()) this.remove(key); this.publish(); }
  dispose(): void { if (this.closed) return; this.closed = true; this.clear(); this.listeners.clear(); }
}
