import { DetailCache, type CacheEntry, type CacheIdentity, type CacheOptions } from '../core/cache.js';
import type { RunsClient } from './client.js';
import type { DefinitionQuery } from './queries.js';
import type { ExecutedDefinition } from './results.js';
import { SyncProtocolError, type RunScope } from './projection.js';
export interface DefinitionRequest extends RunScope { revision?: string | number; query?: Omit<DefinitionQuery, 'workspaceId'> }
function identity(request: DefinitionRequest): CacheIdentity { return { ...request, kind: 'definition' }; }

export class ExecutedDefinitions {
  private readonly cache: DetailCache<ExecutedDefinition>;
  constructor(private readonly runs: RunsClient, options: CacheOptions = {}) { this.cache = new DetailCache({ capacity: options.capacity ?? 120 }); }
  get size(): number { return this.cache.size; }
  subscribe(listener: () => void): () => void { return this.cache.subscribe(listener); }
  entry(request: DefinitionRequest): CacheEntry<ExecutedDefinition> | undefined { return this.cache.entry(identity(request)); }
  load(request: DefinitionRequest): Promise<ExecutedDefinition> {
    const captured = structuredClone(request), key = identity(captured);
    const promise = this.cache.load(key, async signal => {
      const value = await this.runs.definition(captured.runId, { ...captured.query, workspaceId: captured.workspaceId }, signal);
      if (value.runId !== captured.runId || value.exact && (captured.query?.nodePath !== undefined && value.nodePath !== captured.query.nodePath || captured.query?.occurrenceId !== undefined && value.occurrenceId !== captured.query.occurrenceId || captured.query?.hash !== undefined && value.hash !== captured.query.hash)) throw new SyncProtocolError('Incorrect executed definition identity');
      return value;
    });
    // Diagnostic/latest reads may change; only an exact pinned occurrence/hash/revision is reusable.
    void promise.then(value => { if (!value.exact || !(captured.query?.occurrenceId || captured.query?.hash || captured.revision !== undefined)) this.cache.invalidate(key); }, () => {});
    return promise;
  }
  invalidate(request: DefinitionRequest): void { this.cache.invalidate(identity(request)); }
  dispose(): void { this.cache.dispose(); }
}
