import { z } from 'zod';
import { jsonValueSchema, jsonObjectSchema } from '../core/json.js';
export const kindSchema = z.enum(['start', 'end', 'model', 'agent', 'tool', 'set', 'condition', 'input', 'output', 'subgraph', 'context', 'steering', 'inbox', 'route', 'await_route']);
export type Kind = z.output<typeof kindSchema>;
export const retrySettingsSchema = z.strictObject({ maxAttempts: z.number().int().nonnegative().optional(), initialDelayMs: z.number().int().nonnegative().optional(), maxDelayMs: z.number().int().nonnegative().optional(), backoffFactor: z.number().optional(), jitter: z.number().optional(), retryOn: z.string().optional() });
export type RetrySettings = z.output<typeof retrySettingsSchema>;
export const graphSettingsSchema = z.strictObject({ workingDirectory: z.string().nullish(), recursionLimit: z.number().int().nonnegative().optional(), maxConcurrency: z.number().int().nonnegative().nullish(), strictChannels: z.boolean().optional(), timeoutMs: z.number().int().nonnegative().nullish(), idleTimeoutMs: z.number().int().nonnegative().nullish(), retry: retrySettingsSchema.nullish() });
export type GraphSettings = z.output<typeof graphSettingsSchema>;
export const flowNodeSchema = z.object({ id: z.string(), type: z.string(), position: z.object({ x: z.number(), y: z.number() }).catchall(jsonValueSchema), data: z.object({ label: z.string(), kind: z.string(), config: jsonValueSchema }).catchall(jsonValueSchema) }).catchall(jsonValueSchema);
export type FlowNode = z.output<typeof flowNodeSchema>;
export const flowEdgeSchema = z.object({ id: z.string(), source: z.string(), target: z.string(), sourceHandle: z.string().nullish(), targetHandle: z.string().nullish(), label: z.string().nullish() }).catchall(jsonValueSchema);
export type FlowEdge = z.output<typeof flowEdgeSchema>;
export const stateChannelSchema = z.strictObject({ name: z.string(), reducer: z.string(), default: jsonValueSchema.optional() });
export type StateChannel = z.output<typeof stateChannelSchema>;
const compositionDefinition = z.strictObject({ formatVersion: z.number().int().positive().optional(), id: z.string(), name: z.string(), revision: z.number().int(), nodes: z.array(flowNodeSchema), edges: z.array(flowEdgeSchema), settings: graphSettingsSchema.optional(), channels: z.array(stateChannelSchema).optional() });
export interface Composition extends z.output<typeof compositionDefinition> {
}
export const compositionSchema: z.ZodType<Composition, Composition> = compositionDefinition;
export const flowPackageManifestSchema = z.strictObject({ formatVersion: z.literal(1), id: z.string(), name: z.string(), description: z.string().optional(), entry: z.literal('flow.rs'), files: z.array(z.string()), dependencies: z.record(z.string(), z.strictObject({ path: z.string() })).default({}) });
export type FlowPackageManifest = z.output<typeof flowPackageManifestSchema>;
export const packageFileContentSchema = z.union([z.string(), z.strictObject({ base64: z.string() })]);
export const flowPackageNodeSchema = z.strictObject({ manifestSource: z.string(), files: z.record(z.string(), packageFileContentSchema), dependencies: z.record(z.string(), z.string()) });
export const flowPackageSnapshotSchema = z.strictObject({ root: z.string(), packages: z.record(z.string(), flowPackageNodeSchema) });
export type FlowPackageSnapshot = z.output<typeof flowPackageSnapshotSchema>;
export function packageFileBytes(content: z.output<typeof packageFileContentSchema>): Uint8Array<ArrayBuffer> {
    return typeof content === 'string' ? new TextEncoder().encode(content) : Uint8Array.from(atob(content.base64), character => character.charCodeAt(0));
}
/** Inventory the exact captured package bytes; no file reads, source normalization or execution. */
export async function flowPackageInventory(snapshot: FlowPackageSnapshot) {
    return Promise.all(Object.entries(snapshot.packages).map(async ([revision, node]) => {
        const manifest = flowPackageManifestSchema.parse(JSON.parse(node.manifestSource));
        const entries = [{ path: 'flow.json', bytes: new TextEncoder().encode(node.manifestSource) }, ...Object.entries(node.files).map(([path, content]) => ({ path, bytes: packageFileBytes(content) }))];
        const files = await Promise.all(entries.map(async ({ path, bytes }) => ({ path, byteLength: bytes.byteLength, sha256: Array.from(new Uint8Array(await globalThis.crypto.subtle.digest('SHA-256', bytes)), byte => byte.toString(16).padStart(2, '0')).join('') })));
        return { revision, root: revision === snapshot.root, manifest, dependencies: node.dependencies, files };
    }));
}
const flowFileDefinition = z.object({ fileVersion: z.number().int().optional(), key: z.string(), id: z.string(), name: z.string(), path: z.string(), scope: z.enum(['workspace', 'global']), workspaceId: z.string().optional(), hash: z.string(), sourceHash: z.string().optional(), package: flowPackageSnapshotSchema.optional(), composition: compositionSchema.optional(), diagnostics: z.array(z.string()), source: z.string().optional() }).catchall(jsonValueSchema);
export interface FlowFile extends z.output<typeof flowFileDefinition> {
}
export const flowFileSchema: z.ZodType<FlowFile, FlowFile> = flowFileDefinition;
