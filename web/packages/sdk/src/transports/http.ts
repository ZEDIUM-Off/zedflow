import type { RunsClient } from '../runs/client.js';
import type { RunScope } from '../runs/projection.js';
export type Cancel = () => void;
export interface Clock { now(): number; setTimeout(callback: () => void, delayMs: number): Cancel }
/** These globals are resolved only when a lifecycle is explicitly started. */
export const systemClock: Clock = { now: () => Date.now(), setTimeout: (callback, delay) => { const handle = globalThis.setTimeout(callback, delay); return () => globalThis.clearTimeout(handle); } };
export interface Connectivity { online(): boolean; visible(): boolean; subscribe(wake: () => void): Cancel }
export const alwaysConnected: Connectivity = { online: () => true, visible: () => true, subscribe: () => () => {} };
export interface SyncRequest extends RunScope { after?: number; signal: AbortSignal }
export interface RunSyncTransport { snapshot(request: SyncRequest): Promise<unknown> }
export type StreamKind = 'sse' | 'webrtc';
export interface RunStreamObserver { frame(value: unknown, transport: StreamKind): boolean; error(error: unknown): void; traffic?(measurement: { receivedChars: number; decodeMs: number }): void }
export interface RunStreamTransport { subscribe(request: SyncRequest, observer: RunStreamObserver): Cancel }
export function createHttpRunTransport(runs: RunsClient): RunSyncTransport {
  return { snapshot: request => runs.snapshot(request.runId, { workspaceId: request.workspaceId, ...(request.after !== undefined ? { after: request.after } : {}) }, request.signal) };
}
