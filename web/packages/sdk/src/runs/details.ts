import { DetailCache, type CacheEntry, type CacheIdentity, type CacheOptions } from '../core/cache.js';
import type { RunsClient } from './client.js';
import type { ContextProgram } from '../context/model.js';
import type { NodeActivity, ToolActivity, ContextSnapshot, WindowReference } from './model.js';
import type { PassageBoundary, RequestDetail, RunStateDetail, RunMetrics, RunEventPage, TimelinePage, ContextWindowDetail } from './results.js';
import type { ContextProgramQuery, ContextWindowQuery, RunCursorQuery, TimelineQuery } from './queries.js';
import type { RunScope } from './projection.js';
import { SyncProtocolError } from './projection.js';

export interface DetailResults {
  activities: NodeActivity; tools: ToolActivity; context: ContextSnapshot; boundaries: PassageBoundary;
  requests: RequestDetail; state: RunStateDetail; metrics: RunMetrics; 'event-history': RunEventPage;
  timeline: TimelinePage; 'context-windows': WindowReference[]; 'context-window': ContextWindowDetail; 'context-program': ContextProgram;
}
export type DetailKind = keyof DetailResults;
interface DetailParameters {
  activities: { id: string }; tools: { id: string }; context: { id: string }; boundaries: { id: string }; requests: { id: string };
  state: object; metrics: object; 'context-windows': object;
  'event-history': { query?: Omit<RunCursorQuery, 'workspaceId'> };
  timeline: { query?: Omit<TimelineQuery, 'workspaceId'> };
  'context-window': { query: Omit<ContextWindowQuery, 'workspaceId'> };
  'context-program': { query: Omit<ContextProgramQuery, 'workspaceId'> };
}
export type DetailRequest<K extends DetailKind = DetailKind> = { [P in K]: RunScope & { kind: P; revision?: string | number } & DetailParameters[P] }[K];
function identity(request: DetailRequest): CacheIdentity {
  return { workspaceId: request.workspaceId, runId: request.runId, kind: request.kind,
    ...('id' in request ? { id: request.id } : {}), ...(request.revision !== undefined ? { revision: request.revision } : {}),
    ...('query' in request && request.query ? { query: request.query } : {}) };
}

/** Instance-owned details; UI supplies the same descriptor to entry and load. No current-run closure. */
export class RunDetails {
  private readonly cache: DetailCache<DetailResults[DetailKind]>;
  constructor(private readonly runs: RunsClient, options: CacheOptions = {}) { this.cache = new DetailCache(options); }
  get size(): number { return this.cache.size; }
  subscribe(listener: () => void): () => void { return this.cache.subscribe(listener); }
  entry<R extends DetailRequest>(request: R): CacheEntry<DetailResults[R['kind']]> | undefined {
    // The kind is part of the cache key, so this entry can only have been loaded by that kind's branch.
    return this.cache.entry(identity(request)) as CacheEntry<DetailResults[R['kind']]> | undefined;
  }
  load<R extends DetailRequest>(request: R): Promise<DetailResults[R['kind']]> {
    const captured = structuredClone(request);
    return this.cache.load(identity(captured), signal => this.fetch(captured, signal)) as Promise<DetailResults[R['kind']]>;
  }
  private async fetch(request: DetailRequest, signal: AbortSignal): Promise<DetailResults[DetailKind]> {
    const run = request.runId, query = { ...('query' in request ? request.query : {}), workspaceId: request.workspaceId };
    switch (request.kind) {
      case 'activities': { const value = await this.runs.activity(run, request.id, query, signal); if (value.occurrenceId !== request.id) throw new SyncProtocolError('Incorrect activity identity'); return value; }
      case 'tools': { const value = await this.runs.tool(run, request.id, query, signal); if (value.callId !== request.id) throw new SyncProtocolError('Incorrect tool identity'); return value; }
      case 'context': { const value = await this.runs.contextSnapshot(run, request.id, query, signal); if (value.invocationId !== request.id) throw new SyncProtocolError('Incorrect context identity'); return value; }
      case 'boundaries': { const value = await this.runs.boundary(run, request.id, query, signal); if (value.occurrenceId !== request.id) throw new SyncProtocolError('Incorrect boundary identity'); return value; }
      case 'requests': { const value = await this.runs.request(run, request.id, query, signal); if (value.invocationId !== request.id || value.manifest.invocationId !== request.id) throw new SyncProtocolError('Incorrect request identity'); return value; }
      case 'state': return this.runs.state(run, query, signal);
      case 'metrics': return this.runs.metrics(run, query, signal);
      case 'event-history': return this.runs.events(run, query, signal);
      case 'timeline': return this.runs.timeline(run, query, signal);
      case 'context-windows': return this.runs.contextWindows(run, query, signal);
      case 'context-program': return this.runs.contextProgram(run, { ...request.query, workspaceId: request.workspaceId }, signal);
      case 'context-window': {
        const value = await this.runs.contextWindow(run, { ...request.query, workspaceId: request.workspaceId }, signal);
        if (value.nodePath !== request.query.nodePath || value.alias !== request.query.alias || request.query.revision !== undefined && value.revision !== request.query.revision) throw new SyncProtocolError('Incorrect context window identity');
        return value;
      }
    }
  }
  invalidate(request: DetailRequest): void { this.cache.invalidate(identity(request)); }
  clear(): void { this.cache.clear(); }
  dispose(): void { this.cache.dispose(); }
}

/** Reference changes, status changes and completion all invalidate the relevant detail. */
export function detailVersion(value: Readonly<Record<string, unknown>>): string | number {
  if (typeof value.detailRevision === 'string' || typeof value.detailRevision === 'number') return value.detailRevision;
  return JSON.stringify(['argumentsRef', 'inputRef', 'outputRef', 'resultRef', 'contentRef', 'status', 'endedSeq'].map(key => value[key] ?? null));
}
