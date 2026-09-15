import { z } from 'zod';
import { jsonValueSchema, jsonObjectSchema, type JsonValue } from '../core/json.js';
import { contextTypeSchema, contextTypesSchema, sourceFileSchema } from '../context/model.js';
import { compositionSchema } from '../flows/model.js';
export const endpointSchema = z.strictObject({ instance: z.string(), port: z.string() });
export type Endpoint = z.output<typeof endpointSchema>;
export const invocationKindSchema = z.enum(['tool', 'node', 'condition', 'context']);
export type InvocationKind = z.output<typeof invocationKindSchema>;
export const routeModeSchema = z.enum(['callAwait', 'launch', 'handoff']);
export type RouteMode = z.output<typeof routeModeSchema>;
export const dataPermissionSchema = z.strictObject({ read: z.boolean(), write: z.boolean() });
export type DataPermission = z.output<typeof dataPermissionSchema>;
export const portContractSchema = z.strictObject({ input: contextTypeSchema, output: contextTypeSchema.nullish() });
export type PortContract = z.output<typeof portContractSchema>;
export const inferenceDefinitionSchema = z.strictObject({ model: z.discriminatedUnion('kind', [z.strictObject({ kind: z.literal('runtime') }), z.strictObject({ kind: z.literal('fixed'), provider: z.string(), model: z.string() })]), contextStrategy: z.string().nullish(), resources: z.array(z.string()), capabilities: z.array(z.string()) });
export type InferenceDefinition = z.output<typeof inferenceDefinitionSchema>;
const flowDefinitionDefinition = z.strictObject({ entries: z.record(z.string(), portContractSchema), branches: z.record(z.string(), z.strictObject({ contract: portContractSchema, invocations: z.array(invocationKindSchema), requesters: z.array(z.string()).optional() })), data: z.record(z.string(), z.strictObject({ dataType: contextTypeSchema, permissions: dataPermissionSchema })), requires: z.record(z.string(), z.strictObject({ dataType: contextTypeSchema, permissions: dataPermissionSchema, optional: z.boolean().optional() })), inferenceNodes: z.record(z.string(), inferenceDefinitionSchema) });
export interface FlowDefinition extends z.output<typeof flowDefinitionDefinition> {
}
export const flowDefinitionSchema: z.ZodType<FlowDefinition, FlowDefinition> = flowDefinitionDefinition;
const flowExportsDefinition = z.strictObject({ contract: flowDefinitionSchema, types: contextTypesSchema, entries: z.record(z.string(), z.strictObject({ node: z.string(), inputField: z.string(), outputField: z.string().nullish() })), branches: z.record(z.string(), z.string()), data: z.record(z.string(), z.string()), requires: z.record(z.string(), z.string()), interactive: z.boolean() });
export interface FlowExports extends z.output<typeof flowExportsDefinition> {
}
export const flowExportsSchema: z.ZodType<FlowExports, FlowExports> = flowExportsDefinition;
type PredicateNode = {
    kind: 'all' | 'any';
    items: PredicateNode[];
} | {
    kind: 'compare';
    field: string;
    operator: 'eq' | 'ne' | 'gt' | 'gte' | 'lt' | 'lte' | 'exists' | 'contains' | 'in';
    value?: JsonValue | undefined;
};
export const predicateSchema: z.ZodType<PredicateNode, PredicateNode> = z.lazy(() => z.discriminatedUnion('kind', [z.strictObject({ kind: z.enum(['all', 'any']), items: z.array(predicateSchema) }), z.strictObject({ kind: z.literal('compare'), field: z.string(), operator: z.enum(['eq', 'ne', 'gt', 'gte', 'lt', 'lte', 'exists', 'contains', 'in']), value: jsonValueSchema.optional() })]));
export type Predicate = z.output<typeof predicateSchema>;
export const bridgeConnectionSchema = z.strictObject({ from: endpointSchema, to: endpointSchema, mode: routeModeSchema, invocation: invocationKindSchema, toolName: z.string().nullish(), condition: predicateSchema.nullish() });
export type BridgeConnection = z.output<typeof bridgeConnectionSchema>;
export const dataBindingSchema = z.strictObject({ from: endpointSchema, to: endpointSchema, permissions: dataPermissionSchema });
export type DataBinding = z.output<typeof dataBindingSchema>;
const bridgeDefinitionDefinition = z.strictObject({ requires: z.array(z.string()), imports: z.record(z.string(), z.strictObject({ flow: z.string(), reuse: z.string().nullish() })), connections: z.record(z.string(), bridgeConnectionSchema), bindings: z.record(z.string(), dataBindingSchema) });
export interface BridgeDefinition extends z.output<typeof bridgeDefinitionDefinition> {
}
export const bridgeDefinitionSchema: z.ZodType<BridgeDefinition, BridgeDefinition> = bridgeDefinitionDefinition;
const bridgeFileDefinition = sourceFileSchema.extend({ bridge: bridgeDefinitionSchema.optional() });
export interface BridgeFile extends z.output<typeof bridgeFileDefinition> {
}
export const bridgeFileSchema: z.ZodType<BridgeFile, BridgeFile> = bridgeFileDefinition;
export const runtimeSelectionSchema = z.strictObject({ flow: z.string().min(1), entry: z.string().min(1), bridges: z.array(z.string()), flowHashes: z.record(z.string(), z.string()), bridgeHashes: z.record(z.string(), z.string()), contexts: z.record(z.string(), z.strictObject({ key: z.string(), hash: z.string() })).optional() });
export type RuntimeSelection = z.output<typeof runtimeSelectionSchema>;
export const runtimeRouteSchema = bridgeConnectionSchema.extend({ bridge: z.string(), input: contextTypeSchema, output: contextTypeSchema.nullish() });
export type RuntimeRoute = z.output<typeof runtimeRouteSchema>;
export const resolvedDataBindingSchema = dataBindingSchema.extend({ dataType: contextTypeSchema, bridge: z.string() });
export type ResolvedDataBinding = z.output<typeof resolvedDataBindingSchema>;
const runtimeGraphDefinition = z.object({ entry: endpointSchema, types: contextTypesSchema, bridges: z.record(z.string(), bridgeDefinitionSchema), instances: z.record(z.string(), z.object({ flow: z.string(), definition: flowDefinitionSchema })), aliases: z.record(z.string(), z.string()), routes: z.record(z.string(), runtimeRouteSchema), dataBindings: z.record(z.string(), resolvedDataBindingSchema), inferences: z.record(z.string(), z.object({ instance: z.string(), node: z.string(), definition: inferenceDefinitionSchema })) }).catchall(jsonValueSchema);
export interface RuntimeGraph extends z.output<typeof runtimeGraphDefinition> {
}
export const runtimeGraphSchema: z.ZodType<RuntimeGraph, RuntimeGraph> = runtimeGraphDefinition;
const runtimeGraphSummaryDefinition = z.object({ interactive: z.boolean().optional(), types: contextTypesSchema.optional(), entry: endpointSchema, instances: z.record(z.string(), z.object({ flow: z.string(), name: z.string(), hash: z.string(), interactive: z.boolean(), entries: z.record(z.string(), portContractSchema) }).catchall(jsonValueSchema)), inferences: z.record(z.string(), z.object({ instance: z.string(), node: z.string(), label: z.string(), config: jsonObjectSchema, contextProgramHash: z.string().optional(), contextPath: z.string().optional(), context: z.object({ node: z.string(), label: z.string(), config: jsonObjectSchema }).optional() }).catchall(jsonValueSchema)), routes: z.record(z.string(), runtimeRouteSchema), aliases: z.record(z.string(), z.string()), dataBindings: z.record(z.string(), resolvedDataBindingSchema), bridges: z.array(z.string()), bridgeHashes: z.record(z.string(), z.string()).optional() }).catchall(jsonValueSchema);
export interface RuntimeGraphSummary extends z.output<typeof runtimeGraphSummaryDefinition> {
}
export const runtimeGraphSummarySchema: z.ZodType<RuntimeGraphSummary, RuntimeGraphSummary> = runtimeGraphSummaryDefinition;
const preparedRuntimeDefinition = z.object({ definitions: z.object({ flowHashes: z.record(z.string(), z.string()), bridgeHashes: z.record(z.string(), z.string()), bridgeSources: z.record(z.string(), z.string()).optional(), contextSelections: z.record(z.string(), z.object({ key: z.string(), hash: z.string() })).optional() }).catchall(jsonValueSchema), graph: runtimeGraphSchema, flows: z.record(z.string(), z.object({ key: z.string(), hash: z.string(), source: z.string(), composition: compositionSchema, exports: flowExportsSchema }).catchall(jsonValueSchema)) }).catchall(jsonValueSchema);
export interface PreparedRuntime extends z.output<typeof preparedRuntimeDefinition> {
}
export const preparedRuntimeSchema: z.ZodType<PreparedRuntime, PreparedRuntime> = preparedRuntimeDefinition;
const compositionCatalogDefinition = z.strictObject({ types: contextTypesSchema, flows: z.record(z.string(), flowDefinitionSchema), bridges: z.record(z.string(), bridgeDefinitionSchema) });
export interface CompositionCatalog extends z.output<typeof compositionCatalogDefinition> {
}
export const compositionCatalogSchema: z.ZodType<CompositionCatalog, CompositionCatalog> = compositionCatalogDefinition;
