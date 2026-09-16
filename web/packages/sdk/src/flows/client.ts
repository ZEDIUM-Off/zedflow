import { z } from 'zod';
import type { Transport } from '../core/transport.js';
import { validateInput } from '../core/schema.js';
import { jsonValueSchema } from '../core/json.js';
import * as m from './model.js';
import * as c from './commands.js';
import * as q from './queries.js';
import * as r from './results.js';
export function createFlowsClient(transport: Transport) {
    return {
        async list(queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.FlowFile[]> {
            const query = validateInput('flows.list', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'flows.list', method: 'GET', path: `flows`, query, ...(signal ? { signal } : {}) }, z.array(m.flowFileSchema));
        },
        async read(key: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.FlowFile> {
            const keyKey = validateInput('flows.read', z.string().min(1), key);
            const query = validateInput('flows.read', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'flows.read', method: 'GET', path: `flows/${encodeURIComponent(keyKey)}`, query, ...(signal ? { signal } : {}) }, m.flowFileSchema);
        },
        async compositions(queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.Composition[]> {
            const query = validateInput('flows.compositions', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'flows.compositions', method: 'GET', path: `compositions`, query, ...(signal ? { signal } : {}) }, z.array(m.compositionSchema));
        },
        async composition(id: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.Composition> {
            const idKey = validateInput('flows.composition', z.string().min(1), id);
            const query = validateInput('flows.composition', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'flows.composition', method: 'GET', path: `compositions/${encodeURIComponent(idKey)}`, query, ...(signal ? { signal } : {}) }, m.compositionSchema);
        },
        async remove(key: string, queryInput: q.DeleteFlowQuery, signal?: AbortSignal): Promise<r.DeleteFlowResult> {
            const keyKey = validateInput('flows.remove', z.string().min(1), key);
            const query = validateInput('flows.remove', q.deleteFlowQuerySchema, queryInput);
            return transport.json({ operation: 'flows.remove', method: 'DELETE', path: `flows/${encodeURIComponent(keyKey)}`, query, ...(signal ? { signal } : {}) }, r.deleteFlowResultSchema);
        },
        async save(input: c.SaveFlowInput, signal?: AbortSignal): Promise<m.FlowFile> {
            const parsed = validateInput('flows.save', c.saveFlowInputSchema, input);
            const body = validateInput('flows.save', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'flows.save', method: 'POST', path: `flows`, body, ...(signal ? { signal } : {}) }, m.flowFileSchema);
        },
        async convertPackage(input: c.ConvertFlowPackageInput, signal?: AbortSignal): Promise<r.FlowPackageConversionResult> {
            const parsed = validateInput('flows.convertPackage', c.convertFlowPackageInputSchema, input);
            const body = validateInput('flows.convertPackage', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'flows.convertPackage', method: 'POST', path: `flows/convert`, body, ...(signal ? { signal } : {}) }, r.flowPackageConversionResultSchema);
        },
        async convert(input: m.Composition, signal?: AbortSignal): Promise<m.Composition> {
            const parsed = validateInput('flows.convert', m.compositionSchema, input);
            const body = validateInput('flows.convert', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'flows.convert', method: 'POST', path: `flows/convert`, body, ...(signal ? { signal } : {}) }, m.compositionSchema);
        },
        async createComposition(input: m.Composition, signal?: AbortSignal): Promise<m.Composition> {
            const parsed = validateInput('flows.createComposition', m.compositionSchema, input);
            const body = validateInput('flows.createComposition', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'flows.createComposition', method: 'POST', path: `compositions`, body, ...(signal ? { signal } : {}) }, m.compositionSchema);
        },
        async analyze(input: m.Composition, signal?: AbortSignal): Promise<r.GraphAnalysis> {
            const parsed = validateInput('flows.analyze', m.compositionSchema, input);
            const body = validateInput('flows.analyze', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'flows.analyze', method: 'POST', path: `graph-analysis`, body, ...(signal ? { signal } : {}) }, r.graphAnalysisSchema);
        },
        async installWorkingSystemExample(input: c.WorkingSystemExampleInput, signal?: AbortSignal): Promise<r.WorkingSystemExampleResult> {
            const parsed = validateInput('flows.installWorkingSystemExample', c.workingSystemExampleInputSchema, input);
            const body = validateInput('flows.installWorkingSystemExample', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'flows.installWorkingSystemExample', method: 'POST', path: `examples/working-system`, body, ...(signal ? { signal } : {}) }, r.workingSystemExampleResultSchema);
        },
    };
}
export type FlowsClient = ReturnType<typeof createFlowsClient>;
