import { validateResponse } from '../core/schema.js';
import type { Run, TimelineEntry } from './model.js';
import { runSyncSchema, timelinePageSchema, type RunCollection } from './results.js';

export interface RunScope { runId: string; workspaceId: string }
export interface RunProjectionState { readonly run: Run; readonly revision: number; readonly cursor: number }
export interface ProjectionLimits { maxEntities?: number; maxOperations?: number }
export class SyncProtocolError extends Error { constructor(message: string) { super(message); this.name = 'SyncProtocolError'; } }
const collections = ['timeline', 'activities', 'toolActivities', 'contextSnapshots'] as const;
function entityId(collection: RunCollection, entity: Record<string, unknown>): string {
  const field = collection === 'activities' ? 'occurrenceId' : collection === 'toolActivities' ? 'callId' : collection === 'contextSnapshots' ? 'invocationId' : 'id';
  const id = entity[field];
  if (typeof id !== 'string') throw new SyncProtocolError('Missing entity identity');
  return id;
}

/** An incoming frame is validated in full; historical entities are never parsed again for a delta. */
export class RunProjection {
  private current: RunProjectionState | undefined;
  private indexes = new Map<RunCollection, Map<string, number>>();
  private readonly maxEntities: number;
  private readonly maxOperations: number;
  get state(): RunProjectionState | undefined { return this.current; }
  constructor(readonly id: string, readonly workspaceId: string, initial?: RunProjectionState, limits: ProjectionLimits = {}) {
    this.maxEntities = limits.maxEntities ?? 100_000;
    this.maxOperations = limits.maxOperations ?? 10_000;
    if (!id || !workspaceId) throw new SyncProtocolError('Run and workspace identities are required');
    for (const limit of [this.maxEntities, this.maxOperations]) if (!Number.isSafeInteger(limit) || limit < 1) throw new RangeError('Projection limits must be positive integers');
    if (initial) this.apply({ type: 'bootstrap', ...initial });
  }
  private index(values: ReadonlyArray<Record<string, unknown>>, collection: RunCollection): Map<string, number> {
    if (values.length > this.maxEntities) throw new SyncProtocolError('Projection collection limit exceeded');
    const result = new Map<string, number>();
    values.forEach((value, index) => { const id = entityId(collection, value); if (result.has(id)) throw new SyncProtocolError('Duplicate entity in bootstrap'); result.set(id, index); });
    return result;
  }
  prependTimeline(scope: RunScope, input: unknown): void {
    this.checkScope(scope.runId, scope.workspaceId);
    const page = validateResponse('runs.timeline', timelinePageSchema, input);
    if (!this.current) return;
    const known = new Set(this.indexes.get('timeline')?.keys());
    const older: TimelineEntry[] = [];
    for (const entry of page.entries) if (!known.has(entry.id)) { known.add(entry.id); older.push(entry); }
    const timeline = [...older, ...(this.current.run.timeline ?? [])];
    const index = this.index(timeline, 'timeline');
    this.current = { ...this.current, run: Object.assign({}, this.current.run, { timeline, timelineBefore: page.before, timelineHasMore: page.hasMore }) };
    this.indexes.set('timeline', index);
  }
  private checkScope(id: string, workspace: string | undefined): void {
    if (id !== this.id || workspace !== this.workspaceId) throw new SyncProtocolError('Incorrect run or workspace identity');
  }
  apply(input: unknown): { changed: boolean; gap: boolean } {
    const frame = validateResponse('runs.sync', runSyncSchema, input);
    this.checkScope(frame.type === 'bootstrap' ? frame.run.id : frame.runId, frame.type === 'bootstrap' ? frame.run.workspaceId : frame.workspaceId);
    const previous = this.current;
    if (frame.type === 'heartbeat') return { changed: false, gap: !previous || frame.revision > previous.revision || frame.cursor > previous.cursor };
    if (frame.type === 'delta') {
      if (frame.ops.length > this.maxOperations) throw new SyncProtocolError('Delta operation limit exceeded');
      if (frame.revision <= frame.baseRevision) throw new SyncProtocolError('Delta revision must advance');
      for (const operation of frame.ops) {
        if (operation.collection === 'meta') {
          if (operation.value.id !== undefined && operation.value.id !== this.id || operation.value.workspaceId !== undefined && operation.value.workspaceId !== this.workspaceId) throw new SyncProtocolError('Metadata cannot change identity');
        } else if (operation.value && entityId(operation.collection, operation.value) !== operation.id) throw new SyncProtocolError('Incorrect entity identity');
      }
    }
    if (previous && frame.revision <= previous.revision) return { changed: false, gap: false };
    if (previous && frame.cursor < previous.cursor) throw new SyncProtocolError('Cursor cannot move backwards');
    if (frame.type === 'bootstrap') {
      const indexes = new Map<RunCollection, Map<string, number>>();
      for (const collection of collections) indexes.set(collection, this.index(frame.run[collection] ?? [], collection));
      this.current = { run: Object.assign({}, frame.run, { revision: frame.revision }), revision: frame.revision, cursor: frame.cursor };
      this.indexes = indexes;
      return { changed: true, gap: false };
    }
    if (!previous || frame.baseRevision !== previous.revision) return { changed: false, gap: true };
    const next = Object.assign({}, previous.run, { revision: frame.revision });
    const indexes = new Map(this.indexes);
    const touched = new Set<RunCollection>();
    const applyEntity = <T extends Record<string, unknown>>(collection: RunCollection, existing: T[] | undefined, operation: { id: string; value?: T | undefined; delete?: boolean | undefined }): T[] => {
      const values = touched.has(collection) ? existing ?? [] : [...(existing ?? [])];
      if (!touched.has(collection)) { indexes.set(collection, new Map(indexes.get(collection))); touched.add(collection); }
      const index = indexes.get(collection)!;
      const position = index.get(operation.id);
      if (operation.delete) {
        if (position !== undefined) { values.splice(position, 1); indexes.set(collection, this.index(values, collection)); }
      } else if (operation.value) {
        if (position === undefined) { index.set(operation.id, values.length); values.push(operation.value); } else values[position] = operation.value;
        if (values.length > this.maxEntities) throw new SyncProtocolError('Projection collection limit exceeded');
      }
      return values;
    };
    for (const operation of frame.ops) {
      if (operation.collection === 'meta') {
        // Entity collections have their own operations; heavyweight source/state is fetched separately.
        for (const [key, value] of Object.entries(operation.value)) {
          if (![...collections, 'id', 'workspaceId', 'composition', 'flowSource', 'state', 'messages', 'revision'].includes(key)) Object.assign(next, { [key]: value });
        }
      } else switch (operation.collection) {
        case 'timeline': next.timeline = applyEntity('timeline', next.timeline, operation); break;
        case 'activities': next.activities = applyEntity('activities', next.activities, operation); break;
        case 'toolActivities': next.toolActivities = applyEntity('toolActivities', next.toolActivities, operation); break;
        case 'contextSnapshots': next.contextSnapshots = applyEntity('contextSnapshots', next.contextSnapshots, operation); break;
      }
    }
    this.current = { run: next, revision: frame.revision, cursor: frame.cursor }; this.indexes = indexes;
    return { changed: true, gap: false };
  }
}
