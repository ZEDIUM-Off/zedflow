import { jsonValueSchema, jsonObjectSchema } from '../core/json.js';
import { contextProgramSchema, resourceBindingDraftSchema, contextCapabilitySchema, contextTypesSchema, contextLibrarySchema, windowPreparationDraftSchema } from '../context/model.js';
import { flowExportsSchema, flowExportsReadSchema, predicateSchema } from '../composition/model.js';
import { retrySettingsSchema } from './model.js';
import { z } from 'zod';
import { compositionSchema, type Composition } from './model.js';
export const saveFlowInputSchema = z.strictObject({ workspaceId: z.string().min(1).optional(), composition: compositionSchema, scope: z.enum(['workspace', 'global']).optional(), key: z.string().min(1).optional(), expectedHash: z.string().optional() });
export type SaveFlowInput = z.input<typeof saveFlowInputSchema>;
export const workingSystemExampleInputSchema = z.strictObject({ workspaceId: z.string().min(1).optional(), workingDirectory: z.string().min(1) });
export type WorkingSystemExampleInput = z.input<typeof workingSystemExampleInputSchema>;
export const activationSchema = z.enum(['always', 'explicit']);
export type Activation = z.output<typeof activationSchema>;
export const instructionSourceSchema = z.discriminatedUnion('kind', [z.strictObject({ kind: z.literal('text'), text: z.string() }), z.strictObject({ kind: z.literal('file'), path: z.string() }), z.strictObject({ kind: z.literal('workspace') })]);
export type InstructionSource = z.output<typeof instructionSourceSchema>;
export const skillSourceSchema = z.discriminatedUnion('kind', [z.strictObject({ kind: z.literal('file'), path: z.string() }), z.strictObject({ kind: z.literal('workspace') })]);
export type SkillSource = z.output<typeof skillSourceSchema>;
export const instructionItemSchema = z.strictObject({ id: z.string(), enabled: z.boolean().optional(), activation: activationSchema.optional(), source: instructionSourceSchema, mode: z.enum(['literal', 'template']).optional() });
export type InstructionItem = z.output<typeof instructionItemSchema>;
export const skillItemSchema = z.strictObject({ id: z.string(), enabled: z.boolean().optional(), activation: activationSchema.optional(), source: skillSourceSchema, name: z.string().nullish() });
export type SkillItem = z.output<typeof skillItemSchema>;
export const fileItemSchema = z.strictObject({ id: z.string(), enabled: z.boolean().optional(), activation: activationSchema.optional(), path: z.string(), startLine: z.number().int().nonnegative().nullish(), endLine: z.number().int().nonnegative().nullish(), maxChars: z.number().int().nonnegative().optional() });
export type FileItem = z.output<typeof fileItemSchema>;
export const toolItemSchema = z.strictObject({ id: z.string(), enabled: z.boolean().optional(), name: z.string() });
export type ToolItem = z.output<typeof toolItemSchema>;
export const nodeAttachmentsSchema = z.strictObject({ instructions: z.strictObject({ items: z.array(instructionItemSchema) }).optional(), skills: z.strictObject({ items: z.array(skillItemSchema) }).optional(), files: z.strictObject({ items: z.array(fileItemSchema) }).optional(), tools: z.strictObject({ items: z.array(toolItemSchema) }).optional() });
export type NodeAttachments = z.output<typeof nodeAttachmentsSchema>;
/** Editor values retain numeric drafts; API commands still use the constrained item schema. */
export const fileItemDraftSchema = fileItemSchema.extend({ startLine: z.number().nullish(), endLine: z.number().nullish(), maxChars: z.number().optional() });
export const nodeAttachmentsDraftSchema = nodeAttachmentsSchema.extend({ files: z.strictObject({ items: z.array(fileItemDraftSchema) }).optional() });
/** Authored catalogue reference, optionally pinned to an accepted source revision. */
export const sourceReferenceSchema = z.union([z.string(), z.strictObject({ key: z.string(), hash: z.string().nullish() })]);
export type SourceReference = z.output<typeof sourceReferenceSchema>;
/** Known node fields are parsed explicitly by editors; raw catalogues may contain invalid drafts. */
export const nodeConfigSchema = z.object({ modelBinding: z.enum(['fixed', 'runtime']).optional(), provider: z.string().optional(), model: z.string().optional(), reasoningEffort: z.string().nullish(), reasoningSummary: z.string().nullish(), textVerbosity: z.string().nullish(), thinkingBudget: z.number().nullish(), instructions: z.string().optional(), globalInstructions: z.string().optional(), description: z.string().optional(), inputField: z.string().optional(), field: z.string().optional(), historyField: z.string().optional(), toolCallsField: z.string().optional(), tools: z.union([z.array(z.string()), z.strictObject({ items: z.array(toolItemSchema) })]).optional(), temperature: z.number().nullish(), topP: z.number().nullish(), topK: z.number().nullish(), maxOutputTokens: z.number().nullish(), stopSequences: z.array(z.string()).optional(), responseFormat: z.enum(['text', 'json']).optional(), responseSchema: jsonObjectSchema.nullish(), fixtureSteps: z.array(jsonValueSchema).optional(), modelNode: z.string().optional(), contextNode: z.string().nullish(), contextStrategy: sourceReferenceSchema.nullish(), contextLibraryRef: sourceReferenceSchema.nullish(), contextTypesRef: sourceReferenceSchema.nullish(), contextProgram: contextProgramSchema.optional(), contextTypes: contextTypesSchema.optional(), contextLibrary: contextLibrarySchema.optional(), contextWindow: windowPreparationDraftSchema.nullish(), contextBindings: z.record(z.string(), resourceBindingDraftSchema).optional(), contextCapabilities: z.array(z.string()).optional(), capabilityGrants: z.array(contextCapabilitySchema).optional(), attachments: nodeAttachmentsDraftSchema.optional(), windowGrants: z.array(z.strictObject({ alias: z.string(), permission: z.enum(['read', 'write']) })).optional(), composition: compositionSchema.optional(), exports: flowExportsSchema.optional(), retry: retrySettingsSchema.extend({ maxAttempts: z.number().optional(), initialDelayMs: z.number().optional(), maxDelayMs: z.number().optional() }).nullish(), fanIn: z.enum(['all', 'any']).optional(), tool: z.string().optional(), arguments: jsonValueSchema.optional(), ui: z.object({ renderer: z.string().optional(), title: z.string().optional(), language: z.string().optional() }).catchall(jsonValueSchema).optional(), prompt: z.string().optional(), responseType: z.enum(['text', 'confirmation']).optional(), text: z.string().optional(), value: jsonValueSchema.optional(), equals: jsonValueSchema.optional(), predicate: predicateSchema.optional(), branch: z.string().optional(), invocation: z.enum(['tool', 'node', 'condition', 'context']).optional(), routeId: z.string().optional(), fallback: jsonValueSchema.optional() }).catchall(jsonValueSchema);
export type NodeConfig = z.output<typeof nodeConfigSchema>;

/** Validate an editor interpretation while retaining the original mutable object and extensions. */
export function requireNodeConfig(value: unknown): NodeConfig {
    if (!isNodeConfig(value)) throw new Error('Invalid node configuration: ' + nodeConfigSchema.safeParse(value).error?.message);
    return value;
}
export function isNodeConfig(value: unknown): value is NodeConfig {
    return nodeConfigSchema.safeParse(value).success;
}

export const convertFlowPackageInputSchema = z.strictObject({ workspaceId: z.string().min(1).optional(), key: z.string().min(1), expectedHash: z.string().min(1) });
export type ConvertFlowPackageInput = z.input<typeof convertFlowPackageInputSchema>;

/** Normalize only a detached editor copy; catalogue source and captured revisions stay exact. */
export function editableComposition(input: Composition): Composition {
    const copy = compositionSchema.parse(JSON.parse(JSON.stringify(input)));
    for (const node of copy.nodes) {
        const config = jsonObjectSchema.parse(node.data.config);
        if (config.exports !== undefined) config.exports = jsonValueSchema.parse(flowExportsReadSchema.parse(config.exports));
        if (config.composition !== undefined) config.composition = jsonValueSchema.parse(editableComposition(compositionSchema.parse(config.composition)));
        node.data.config = config;
        requireNodeConfig(config);
    }
    return copy;
}
