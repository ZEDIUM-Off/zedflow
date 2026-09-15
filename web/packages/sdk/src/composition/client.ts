import { z } from 'zod';
import type { Transport } from '../core/transport.js';
import { validateInput } from '../core/schema.js';
import { jsonValueSchema } from '../core/json.js';
import * as m from './model.js';
import * as c from './commands.js';
import * as q from './queries.js';
import * as r from './results.js';
export function createCompositionClient(transport: Transport) {
    return {
        async listBridges(queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.BridgeFile[]> {
            const query = validateInput('composition.listBridges', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'composition.listBridges', method: 'GET', path: `bridges`, query, ...(signal ? { signal } : {}) }, z.array(m.bridgeFileSchema));
        },
        async readBridge(key: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.BridgeFile> {
            const keyKey = validateInput('composition.readBridge', z.string().min(1), key);
            const query = validateInput('composition.readBridge', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'composition.readBridge', method: 'GET', path: `bridges/${encodeURIComponent(keyKey)}`, query, ...(signal ? { signal } : {}) }, m.bridgeFileSchema);
        },
        async saveBridge(input: c.SaveBridgeInput, signal?: AbortSignal): Promise<m.BridgeFile> {
            const parsed = validateInput('composition.saveBridge', c.saveBridgeInputSchema, input);
            const body = validateInput('composition.saveBridge', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'composition.saveBridge', method: 'POST', path: `bridges`, body, ...(signal ? { signal } : {}) }, m.bridgeFileSchema);
        },
        async analyze(input: c.AnalyzeCompositionInput, signal?: AbortSignal): Promise<r.CompositionAnalysis> {
            const parsed = validateInput('composition.analyze', c.analyzeCompositionInputSchema, input);
            const body = validateInput('composition.analyze', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'composition.analyze', method: 'POST', path: `composition-analysis`, body, ...(signal ? { signal } : {}) }, r.compositionAnalysisSchema);
        },
        async resolve(input: c.ResolveCompositionInput, signal?: AbortSignal): Promise<r.ResolvedComposition> {
            const parsed = validateInput('composition.resolve', c.resolveCompositionInputSchema, input);
            const body = validateInput('composition.resolve', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'composition.resolve', method: 'POST', path: `runtime-graphs/resolve`, body, ...(signal ? { signal } : {}) }, r.resolvedCompositionSchema);
        },
        async prepare(input: c.PrepareRuntimeInput, signal?: AbortSignal): Promise<r.PreparedComposition> {
            const parsed = validateInput('composition.prepare', c.prepareRuntimeInputSchema, input);
            const body = validateInput('composition.prepare', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'composition.prepare', method: 'POST', path: `runtime-graphs/prepare`, body, ...(signal ? { signal } : {}) }, r.preparedCompositionSchema);
        },
    };
}
export type CompositionClient = ReturnType<typeof createCompositionClient>;
