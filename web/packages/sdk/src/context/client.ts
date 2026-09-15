import { z } from 'zod';
import type { Transport } from '../core/transport.js';
import { validateInput } from '../core/schema.js';
import { jsonValueSchema } from '../core/json.js';
import * as m from './model.js';
import * as c from './commands.js';
import * as q from './queries.js';
import * as r from './results.js';
export function createContextClient(transport: Transport) {
    return {
        async convert(input: c.ConvertContextInput, signal?: AbortSignal): Promise<r.ContextConversion> {
            const parsed = validateInput('context.convert', c.convertContextInputSchema, input);
            const body = validateInput('context.convert', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'context.convert', method: 'POST', path: 'context-strategies/convert', body, ...(signal ? { signal } : {}) }, r.contextConversionSchema);
        },
        async exportPackage(input: c.ExportContextPackageInput, signal?: AbortSignal): Promise<m.ContextPackage> {
            const parsed = validateInput('context.exportPackage', c.exportContextPackageInputSchema, input);
            const body = validateInput('context.exportPackage', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'context.exportPackage', method: 'POST', path: 'context-packages/export', body, ...(signal ? { signal } : {}) }, m.contextPackageSchema);
        },
        async validatePackage(input: c.ContextPackageInput, signal?: AbortSignal): Promise<r.ContextPackageValidation> {
            const parsed = validateInput('context.validatePackage', c.contextPackageInputSchema, input);
            const body = validateInput('context.validatePackage', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'context.validatePackage', method: 'POST', path: 'context-packages/validate', body, ...(signal ? { signal } : {}) }, r.contextPackageValidationSchema);
        },
        async importPackage(input: c.ContextPackageInput, signal?: AbortSignal): Promise<r.ContextPackageImport> {
            const parsed = validateInput('context.importPackage', c.contextPackageInputSchema, input);
            const body = validateInput('context.importPackage', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'context.importPackage', method: 'POST', path: 'context-packages/import', body, ...(signal ? { signal } : {}) }, r.contextPackageImportSchema);
        },
        async queryExamples(input: c.TypeExampleQuery, signal?: AbortSignal): Promise<m.TypeExample[]> {
            const parsed = validateInput('context.queryExamples', c.typeExampleQuerySchema, input);
            const body = validateInput('context.queryExamples', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'context.queryExamples', method: 'POST', path: 'type-examples/query', body, ...(signal ? { signal } : {}) }, z.array(m.typeExampleSchema));
        },
        async saveExample(input: c.SaveTypeExampleInput, signal?: AbortSignal): Promise<m.TypeExample> {
            const parsed = validateInput('context.saveExample', c.saveTypeExampleInputSchema, input);
            const body = validateInput('context.saveExample', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'context.saveExample', method: 'POST', path: 'type-examples', body, ...(signal ? { signal } : {}) }, m.typeExampleSchema);
        },
        async readers(input: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.ReaderContract[]> {
            const query = validateInput('context.readers', q.workspaceQuerySchema, input);
            return transport.json({ operation: 'context.readers', method: 'GET', path: 'context-readers', query, ...(signal ? { signal } : {}) }, z.array(m.readerContractSchema));
        },
        async sourceTypes(input: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.SourceCatalog> {
            const query = validateInput('context.sourceTypes', q.workspaceQuerySchema, input);
            return transport.json({ operation: 'context.sourceTypes', method: 'GET', path: 'context-source-types', query, ...(signal ? { signal } : {}) }, r.sourceCatalogSchema);
        },
        async exampleCatalog(input: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.SourceFile[]> {
            const query = validateInput('context.exampleCatalog', q.workspaceQuerySchema, input);
            return transport.json({ operation: 'context.exampleCatalog', method: 'GET', path: 'type-examples/catalog', query, ...(signal ? { signal } : {}) }, z.array(m.sourceFileSchema));
        },
        async list(queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.ContextFile[]> {
            const query = validateInput('context.list', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'context.list', method: 'GET', path: `context-strategies`, query, ...(signal ? { signal } : {}) }, z.array(m.contextFileSchema));
        },
        async read(key: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.ContextFile> {
            const keyKey = validateInput('context.read', z.string().min(1), key);
            const query = validateInput('context.read', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'context.read', method: 'GET', path: `context-strategies/${encodeURIComponent(keyKey)}`, query, ...(signal ? { signal } : {}) }, m.contextFileSchema);
        },
        async listLibraries(queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.ContextLibraryFile[]> {
            const query = validateInput('context.listLibraries', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'context.listLibraries', method: 'GET', path: `context-libraries`, query, ...(signal ? { signal } : {}) }, z.array(m.contextLibraryFileSchema));
        },
        async readLibrary(key: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.ContextLibraryFile> {
            const keyKey = validateInput('context.readLibrary', z.string().min(1), key);
            const query = validateInput('context.readLibrary', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'context.readLibrary', method: 'GET', path: `context-libraries/${encodeURIComponent(keyKey)}`, query, ...(signal ? { signal } : {}) }, m.contextLibraryFileSchema);
        },
        async listTypes(queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.ContextTypesFile[]> {
            const query = validateInput('context.listTypes', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'context.listTypes', method: 'GET', path: `context-types`, query, ...(signal ? { signal } : {}) }, z.array(m.contextTypesFileSchema));
        },
        async readTypes(key: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.ContextTypesFile> {
            const keyKey = validateInput('context.readTypes', z.string().min(1), key);
            const query = validateInput('context.readTypes', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'context.readTypes', method: 'GET', path: `context-types/${encodeURIComponent(keyKey)}`, query, ...(signal ? { signal } : {}) }, m.contextTypesFileSchema);
        },
        async save(input: c.SaveContextInput, signal?: AbortSignal): Promise<m.ContextFile> {
            const parsed = validateInput('context.save', c.saveContextInputSchema, input);
            const body = validateInput('context.save', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'context.save', method: 'POST', path: `context-strategies`, body, ...(signal ? { signal } : {}) }, m.contextFileSchema);
        },
        async saveLibrary(input: c.SaveContextLibraryInput, signal?: AbortSignal): Promise<m.ContextLibraryFile> {
            const parsed = validateInput('context.saveLibrary', c.saveContextLibraryInputSchema, input);
            const body = validateInput('context.saveLibrary', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'context.saveLibrary', method: 'POST', path: `context-libraries`, body, ...(signal ? { signal } : {}) }, m.contextLibraryFileSchema);
        },
        async saveTypes(input: c.SaveContextTypesInput, signal?: AbortSignal): Promise<m.ContextTypesFile> {
            const parsed = validateInput('context.saveTypes', c.saveContextTypesInputSchema, input);
            const body = validateInput('context.saveTypes', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'context.saveTypes', method: 'POST', path: `context-types`, body, ...(signal ? { signal } : {}) }, m.contextTypesFileSchema);
        },
        async preview(input: c.PreviewContextInput, signal?: AbortSignal): Promise<r.ContextPreview> {
            const parsed = validateInput('context.preview', c.previewContextInputSchema, input);
            const body = validateInput('context.preview', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'context.preview', method: 'POST', path: `context-strategies/preview`, body, ...(signal ? { signal } : {}) }, r.contextPreviewSchema);
        },
        async validate(input: c.ValidateContextInput, signal?: AbortSignal): Promise<r.ContextValidation> {
            const parsed = validateInput('context.validate', c.validateContextInputSchema, input);
            const body = validateInput('context.validate', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'context.validate', method: 'POST', path: `context-strategies/validate`, body, ...(signal ? { signal } : {}) }, r.contextValidationSchema);
        },
        async workspace(queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.WorkspaceContext> {
            const query = validateInput('context.workspace', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'context.workspace', method: 'GET', path: `context`, query, ...(signal ? { signal } : {}) }, m.workspaceContextSchema);
        },
    };
}
export type ContextClient = ReturnType<typeof createContextClient>;
