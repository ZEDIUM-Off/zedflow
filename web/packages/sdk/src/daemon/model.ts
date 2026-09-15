import { z } from 'zod';
import { jsonValueSchema, jsonObjectSchema } from '../core/json.js';
export const buildInfoSchema = z.object({
    component: z.enum(['client', 'daemon']), version: z.string(), buildId: z.string(),
    protocol: z.number().int().nonnegative(), storageEpoch: z.number().int().nonnegative(),
    revision: z.string().nullable().optional(), target: z.string().optional(),
}).catchall(jsonValueSchema);
export type BuildInfo = z.output<typeof buildInfoSchema>;
export const releaseSchema = z.object({
    releaseId: z.string(), createdAt: z.string(), daemon: buildInfoSchema, client: buildInfoSchema,
}).catchall(jsonValueSchema);
export type Release = z.output<typeof releaseSchema>;
export const daemonVersionSchema = z.object({
    daemon: buildInfoSchema, client: buildInfoSchema.nullable(), releaseId: z.string().nullable(),
    managed: z.boolean(), candidate: releaseSchema.nullable(), previous: releaseSchema.nullable(),
    maintenance: z.boolean(), activeExecutions: z.number().int().optional(), catalogError: z.string().optional(),
    operation: z.object({ phase: z.string().optional(), error: z.string().optional() }).catchall(jsonValueSchema).nullable().optional(),
    daemonId: z.string().optional(), instanceId: z.string().optional(), channel: z.string().optional(),
}).catchall(jsonValueSchema);
export type DaemonVersion = z.output<typeof daemonVersionSchema>;
export const daemonHealthSchema = z.object({
    name: z.string().optional(),
    daemon: z.object({ id: z.string(), host: z.string(), name: z.string().optional(), version: z.string().optional(), instanceId: z.string().optional(), endpoint: z.string().optional(), build: buildInfoSchema.optional() }).catchall(jsonValueSchema).optional(),
    workspace: z.object({ id: z.string().optional(), host: z.string(), path: z.string() }).catchall(jsonValueSchema).optional(),
    defaultWorkspaceId: z.string().optional(), adk: z.string().optional(), capabilities: z.array(z.string()).optional(),
}).catchall(jsonValueSchema);
export type DaemonHealth = z.output<typeof daemonHealthSchema>;
export const capabilityFieldSchema = z.object({ type: z.string().optional(), default: jsonValueSchema.optional(), minimum: z.number().optional(), maximum: z.number().optional(), label: z.string().optional(), description: z.string().optional(), enum: z.array(jsonValueSchema).optional() }).catchall(jsonValueSchema);
export type CapabilityField = z.output<typeof capabilityFieldSchema>;
export const capabilityInventorySchema = z.object({
    adkVersion: z.string(),
    graph: z.object({ settings: z.record(z.string(), capabilityFieldSchema), channelReducers: z.array(z.string()), builtInChannels: z.array(z.string()), retryFields: z.record(z.string(), capabilityFieldSchema) }).catchall(jsonValueSchema),
    nodes: z.array(z.object({ kind: z.string(), label: z.string(), fields: z.array(z.string()), supported: z.boolean(), authorable: z.boolean().optional(), description: z.string().optional(), responseTypes: z.array(z.string()).optional() }).catchall(jsonValueSchema)),
    tools: z.record(z.string(), z.object({ name: z.string(), description: z.string(), parameters: jsonObjectSchema }).catchall(jsonValueSchema)),
    nodePolicies: z.array(z.string()), renderers: z.array(z.string()), toolDispatch: z.string(), toolDispatchModes: z.array(z.string()),
    harness: z.object({ modelBindings: z.array(z.string()).optional(), queueKinds: z.array(z.string()).optional(), abort: z.boolean().optional(), resume: z.boolean().optional(), skills: z.string().optional(), compaction: z.boolean().optional(), filesystemScope: z.string().optional() }).catchall(jsonValueSchema), fanIn: z.object({ default: z.string().optional(), enum: z.array(z.string()).optional(), description: z.string().optional() }).catchall(jsonValueSchema),
    providers: z.array(z.object({ id: z.string(), label: z.string(), credentials: z.union([z.string(), z.boolean()]), supportsSampling: z.boolean(), supportsResponseSchema: z.boolean().optional(), fields: z.array(z.string()).optional(), description: z.string().optional() }).catchall(jsonValueSchema)),
    unsupported: z.array(z.object({ id: z.string(), reason: z.string() }).catchall(jsonValueSchema)),
}).catchall(jsonValueSchema);
export type CapabilityInventory = z.output<typeof capabilityInventorySchema>;
export const rtcConfigSchema = z.object({ iceServers: z.array(z.object({ urls: z.array(z.string()), username: z.string(), credential: z.string() }).catchall(jsonValueSchema)), iceTransportPolicy: z.enum(['all', 'relay']) }).catchall(jsonValueSchema);
export type RtcConfig = z.output<typeof rtcConfigSchema>;
