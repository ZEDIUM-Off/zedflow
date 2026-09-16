import { z } from 'zod';
import { jsonValueSchema, jsonObjectSchema } from '../core/json.js';
import { compositionSchema } from '../flows/model.js';
import { runtimeSelectionSchema, runtimeGraphSummarySchema } from '../composition/model.js';
import { historicalModelSelectionSchema } from '../models/model.js';
import { sessionImportInfoSchema } from '../sessions/model.js';
import * as context from '../context/model.js';
import { contextEvaluationSchema, contextItemSchema } from '../context/results.js';
export const timelineOriginSchema = z.object({ nodePath: z.string(), occurrenceId: z.string() }).catchall(jsonValueSchema);
export type TimelineOrigin = z.output<typeof timelineOriginSchema>;
export const flowRevisionSchema = z.object({ instance: z.string(), scope: z.string(), threadId: z.string(), step: z.number().int(), key: z.string(), hash: z.string(), graphRef: z.string().optional(), sourceRef: z.string().optional(), definitionRef: z.string().optional(), diagnostic: z.object({ code: z.string(), message: z.string().optional() }).catchall(jsonValueSchema).nullish() }).catchall(jsonValueSchema);
export type FlowRevision = z.output<typeof flowRevisionSchema>;
const nodeActivityDefinition = z.object({ occurrenceId: z.string(), node: z.string(), path: z.string(), label: z.string(), kind: z.string(), step: z.number().int(), status: z.enum(['running', 'completed', 'waiting', 'error', 'interrupted', 'resumed']), startedAt: z.number(), endedAt: z.number().nullish(), durationMs: z.number().nullish(), flowRevision: flowRevisionSchema.nullish(), gapBeforeMs: z.number().optional(), inputRef: z.string().optional(), outputRef: z.string().optional(), stateRef: z.string().optional(), detailRevision: z.union([z.string(), z.number()]).optional(), startedSeq: z.number().int().optional(), endedSeq: z.number().int().optional(), input: jsonValueSchema.optional(), output: z.lazy(() => runStateSchema).nullish(), error: z.string().nullish(), tool: z.string().nullish(), ui: z.object({ renderer: z.string().optional(), title: z.string().optional(), language: z.string().optional() }).catchall(jsonValueSchema).optional(), modelSelection: historicalModelSelectionSchema.optional() }).catchall(jsonValueSchema);
export interface NodeActivity extends z.output<typeof nodeActivityDefinition> {
}
export const nodeActivitySchema: z.ZodType<NodeActivity, NodeActivity> = nodeActivityDefinition;
const toolActivityDefinition = z.object({ callId: z.string(), id: z.string().optional(), nodePath: z.string().nullish(), name: z.string().nullish(), status: z.string().optional(), origin: timelineOriginSchema.nullish(), arguments: jsonValueSchema.optional(), result: jsonValueSchema.optional(), output: z.string().optional(), error: jsonValueSchema.optional(), argumentsRef: z.string().optional(), resultRef: z.string().optional(), outputRef: z.string().optional(), argumentsPreview: jsonValueSchema.optional(), receiptRef: z.string().optional(), fullOutputRef: z.string().optional(), startedAt: z.number().nullish(), endedAt: z.number().nullish(), durationMs: z.number().nullish(), truncated: z.boolean().nullish(), detailRevision: z.union([z.string(), z.number()]).optional() }).catchall(jsonValueSchema);
export interface ToolActivity extends z.output<typeof toolActivityDefinition> {
}
export const toolActivitySchema: z.ZodType<ToolActivity, ToolActivity> = toolActivityDefinition;
export const timelineEntrySchema = z.discriminatedUnion('kind', [z.object({ id: z.string(), seq: z.number().int(), origin: timelineOriginSchema.optional(), approximate: z.boolean().optional(), kind: z.literal('message'), role: z.enum(['user', 'assistant']), text: z.string() }).catchall(jsonValueSchema), z.object({ id: z.string(), seq: z.number().int(), origin: timelineOriginSchema.optional(), approximate: z.boolean().optional(), kind: z.literal('tool'), activity: toolActivitySchema }).catchall(jsonValueSchema)]);
export type TimelineEntry = z.output<typeof timelineEntrySchema>;
export const queuedMessageSchema = z.object({ id: z.string(), kind: z.enum(['steering', 'followup']), text: z.string(), status: z.enum(['pending', 'consumed', 'cancelled']), originalText: z.string().optional(), nodePath: z.string().optional() }).catchall(jsonValueSchema);
export type QueuedMessage = z.output<typeof queuedMessageSchema>;
export const waitKindSchema = z.enum(['input', 'model_selection', 'stopped', 'context_production', 'context_window_preparation']);
export type WaitKind = z.output<typeof waitKindSchema>;
const runWaitDefinition = z.object({ id: z.string(), kind: z.string().optional(), node: z.string(), nodePath: z.string().optional(), occurrenceId: z.string().optional(), config: z.object({ prompt: z.string().optional(), responseType: z.string().optional() }).catchall(jsonValueSchema) }).catchall(jsonValueSchema);
export interface RunWait extends z.output<typeof runWaitDefinition> {
}
export const runWaitSchema: z.ZodType<RunWait, RunWait> = runWaitDefinition;
export const windowGrantSchema = z.strictObject({ alias: z.string(), permission: z.enum(['read', 'write']) });
export type WindowGrant = z.output<typeof windowGrantSchema>;
const preparedWindowDefinition = z.strictObject({ strategyId: z.string(), strategyRevision: z.string(), programRevision: z.string().optional(), items: z.array(contextItemSchema), sourceRevisions: z.record(z.string(), z.string()), capabilities: z.array(context.contextCapabilitySchema) });
export interface PreparedWindow extends z.output<typeof preparedWindowDefinition> {
}
export const preparedWindowSchema: z.ZodType<PreparedWindow, PreparedWindow> = preparedWindowDefinition;
export const windowReferenceSchema = z.object({ nodePath: z.string(), alias: z.string(), entityId: z.string(), revision: z.string(), programHash: z.string() }).catchall(jsonValueSchema);
export type WindowReference = z.output<typeof windowReferenceSchema>;
export const capturedResourceSchema = z.union([z.object({ name: z.string(), value: jsonValueSchema, provenance: jsonObjectSchema }).catchall(jsonValueSchema), z.object({ id: z.string(), kind: z.string(), path: z.string().nullish(), hash: z.string().optional(), content: z.string().optional(), contentRef: z.string().optional() }).catchall(jsonValueSchema)]);
export type CapturedResource = z.output<typeof capturedResourceSchema>;
export const capturedSkillSchema = z.object({ name: z.string(), description: z.string(), path: z.string(), itemId: z.string().optional(), manualOnly: z.boolean().optional(), activationKey: z.string().optional() }).catchall(jsonValueSchema);
export type CapturedSkill = z.output<typeof capturedSkillSchema>;
const preparedContextSnapshotDefinition = z.object({ version: z.number().int(), program: context.contextProgramSchema, flowRevision: flowRevisionSchema.nullish(), evaluation: contextEvaluationSchema, resourceStatus: z.record(z.string(), jsonObjectSchema).optional(), windowGrants: z.array(windowGrantSchema), adapter: z.object({ kind: z.literal('llm'), provider: z.string(), error: z.string().optional() }).catchall(jsonValueSchema), window: z.object({ alias: z.string(), revision: z.string() }).catchall(jsonValueSchema).optional(), consumedItems: z.array(contextItemSchema).optional() }).catchall(jsonValueSchema);
export interface PreparedContextSnapshot extends z.output<typeof preparedContextSnapshotDefinition> {
}
export const preparedContextSnapshotSchema: z.ZodType<PreparedContextSnapshot, PreparedContextSnapshot> = preparedContextSnapshotDefinition;
const contextSnapshotDefinition = z.object({ invocationId: z.string(), agentPath: z.string().optional(), nodePath: z.string().optional(), origin: timelineOriginSchema.nullish(), tools: z.array(z.string()).optional(), system: z.string().optional(), files: z.string().optional(), resources: z.array(capturedResourceSchema).optional(), skillCatalog: z.array(capturedSkillSchema).optional(), prepared: preparedContextSnapshotSchema.optional(), contentRef: z.string().optional(), skillCatalogRef: z.string().optional(), requestRef: z.string().optional(), rawRef: z.string().optional(), requestBoundary: z.string().optional(), requestStatus: z.enum(['prepared', 'sent', 'unavailable']).optional() }).catchall(jsonValueSchema);
export interface ContextSnapshot extends z.output<typeof contextSnapshotDefinition> {
}
export const contextSnapshotSchema: z.ZodType<ContextSnapshot, ContextSnapshot> = contextSnapshotDefinition;
export const flowReferenceSchema = z.object({ hash: z.string(), key: z.string().optional(), path: z.string().optional(), id: z.string().optional(), name: z.string().optional(), fileVersion: z.number().int().optional(), scope: z.enum(['workspace', 'global']).optional() }).catchall(jsonValueSchema);
export type FlowReference = z.output<typeof flowReferenceSchema>;
export const runPreviewSchema = z.object({ sourceWorkspaceId: z.string(), sourceWorkspacePath: z.string(), sourceCompositionRef: z.string().optional(), temporaryWorkspace: z.boolean() }).catchall(jsonValueSchema);
export type RunPreview = z.output<typeof runPreviewSchema>;
export const runSummarySchema = z.object({ id: z.string(), name: z.string(), status: z.string(), workspaceId: z.string().optional(), workspacePath: z.string().optional(), createdAt: z.number().optional(), updatedAt: z.number().optional(), flowRef: flowReferenceSchema.optional(), error: z.string().nullish(), interactive: z.boolean().optional(), runtimeActive: z.boolean().optional() }).catchall(jsonValueSchema);
export type RunSummary = z.output<typeof runSummarySchema>;
export const runContextSchema = context.workspaceContextSchema.extend({ instructions: z.array(z.object({ path: z.string(), hash: z.string(), content: z.string().optional() }).catchall(jsonValueSchema)) });
export type RunContext = z.output<typeof runContextSchema>;
export const runDefinitionSchema = runSummarySchema.extend({ composition: compositionSchema, state: jsonObjectSchema, messages: z.array(z.object({ id: z.string().optional(), role: z.enum(['user', 'assistant']), text: z.string() }).catchall(jsonValueSchema)), wait: runWaitSchema.nullable(), preview: runPreviewSchema.optional(), runtimeGraphSummary: runtimeGraphSummarySchema.optional(), runtimeSelection: runtimeSelectionSchema.nullish(), hasFlowSource: z.boolean().optional(), flowSource: z.string().optional(), timelineBefore: z.number().int().nullish(), timelineHasMore: z.boolean().optional(), revision: z.number().int().optional(), capabilityActivations: z.record(z.string(), z.array(z.string())).optional(), contextSnapshots: z.array(contextSnapshotSchema).optional(), import: sessionImportInfoSchema.optional(), timeline: z.array(timelineEntrySchema).optional(), timelineVersion: z.number().int().optional(), timelineApproximate: z.boolean().optional(), activeNode: z.string().nullish(), activeNodes: z.array(z.string()).optional(), activities: z.array(nodeActivitySchema).optional(), checkpoint: z.string().nullish(), modelBindings: z.record(z.string(), historicalModelSelectionSchema).optional(), modelRevision: z.number().int().optional(), queue: z.array(queuedMessageSchema).optional(), context: runContextSchema.optional(), toolActivities: z.array(toolActivitySchema).optional() });
export interface Run extends z.output<typeof runDefinitionSchema> {
}
export const runSchema: z.ZodType<Run, Run> = runDefinitionSchema;
import { modelResponseSchema, toolCallSchema, toolResultSchema, adkContentSchema } from '../models/model.js';
export const runStateSchema = z.object({ modelResponse: modelResponseSchema.extend({ effectiveContext: contextSnapshotSchema.optional() }).optional(), toolCalls: z.array(toolCallSchema).optional(), toolResults: z.array(toolResultSchema).optional(), messages: z.array(adkContentSchema).optional(), hasToolCalls: z.boolean().optional(), hasSteering: z.boolean().optional(), hasFollowUp: z.boolean().optional(), input: jsonValueSchema.optional(), output: jsonValueSchema.optional(), response: jsonValueSchema.optional() }).catchall(jsonValueSchema);
export type RunState = z.output<typeof runStateSchema>;
