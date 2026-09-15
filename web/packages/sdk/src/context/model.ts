import { z } from 'zod';
import { jsonValueSchema, jsonObjectSchema, type JsonValue } from '../core/json.js';
// Recursive anchors preserve named references in declarations; domain types derive from validators.
export type TypeNode = {
    kind: 'boolean' | 'number' | 'text';
} | {
    kind: 'list';
    item: TypeNode;
} | {
    kind: 'record';
    fields: Record<string, TypeNode>;
} | {
    kind: 'media';
    mediaType: string;
} | {
    kind: 'named';
    name: string;
};
export const contextTypeSchema: z.ZodType<TypeNode, TypeNode> = z.lazy(() => z.discriminatedUnion('kind', [
    z.strictObject({ kind: z.enum(['boolean', 'number', 'text']) }), z.strictObject({ kind: z.literal('list'), item: contextTypeSchema }), z.strictObject({ kind: z.literal('record'), fields: z.record(z.string(), contextTypeSchema) }), z.strictObject({ kind: z.literal('media'), mediaType: z.string() }), z.strictObject({ kind: z.literal('named'), name: z.string() }),
]));
export type ContextType = z.output<typeof contextTypeSchema>;
export type ExprNode = {
    kind: 'resource' | 'variable';
    name: string;
} | {
    kind: 'field';
    value: ExprNode;
    field: string;
} | {
    kind: 'project';
    value: ExprNode;
    fields: string[];
} | {
    kind: 'literal';
    dataType: TypeNode;
    value: JsonValue;
} | {
    kind: 'filter';
    value: ExprNode;
    item: string;
    condition: PredicateNode;
} | {
    kind: 'sort';
    value: ExprNode;
    item: string;
    key: ExprNode;
    descending: boolean;
} | {
    kind: 'take' | 'truncate';
    value: ExprNode;
    count: number;
} | {
    kind: 'map';
    value: ExprNode;
    item: string;
    body: ExprNode;
} | {
    kind: 'groupBy' | 'dedup';
    value: ExprNode;
    item: string;
    key: ExprNode;
} | {
    kind: 'record';
    fields: Record<string, ExprNode>;
} | {
    kind: 'list';
    itemType: TypeNode;
    items: ExprNode[];
} | {
    kind: 'template';
    template: string;
    values: Record<string, ExprNode>;
} | {
    kind: 'toJson';
    value: ExprNode;
} | {
    kind: 'construct';
    name: string;
    value: ExprNode;
} | {
    kind: 'measure';
    value: ExprNode;
    unit: 'bytes' | 'items' | 'media';
} | {
    kind: 'call';
    catalog: 'projection' | 'subprogram';
    name: string;
    arguments: Record<string, ExprNode>;
};
export type PredicateNode = {
    kind: 'present';
    value: ExprNode;
} | {
    kind: 'eq';
    left: ExprNode;
    right: ExprNode;
} | {
    kind: 'compare';
    left: ExprNode;
    operator: 'lt' | 'lte' | 'gt' | 'gte' | 'ne';
    right: ExprNode;
} | {
    kind: 'contains';
    value: ExprNode;
    item: ExprNode;
} | {
    kind: 'and' | 'or';
    items: PredicateNode[];
} | {
    kind: 'not';
    item: PredicateNode;
};
export const contextExprSchema: z.ZodType<ExprNode, ExprNode> = z.lazy(() => z.discriminatedUnion('kind', [
    z.strictObject({ kind: z.enum(['resource', 'variable']), name: z.string() }),
    z.strictObject({ kind: z.literal('field'), value: contextExprSchema, field: z.string() }),
    z.strictObject({ kind: z.literal('project'), value: contextExprSchema, fields: z.array(z.string()) }),
    z.strictObject({ kind: z.literal('literal'), dataType: contextTypeSchema, value: jsonValueSchema }),
    z.strictObject({ kind: z.literal('filter'), value: contextExprSchema, item: z.string(), condition: contextPredicateSchema }),
    z.strictObject({ kind: z.literal('sort'), value: contextExprSchema, item: z.string(), key: contextExprSchema, descending: z.boolean() }),
    z.strictObject({ kind: z.enum(['take', 'truncate']), value: contextExprSchema, count: z.number().int().nonnegative() }),
    z.strictObject({ kind: z.literal('map'), value: contextExprSchema, item: z.string(), body: contextExprSchema }),
    z.strictObject({ kind: z.enum(['groupBy', 'dedup']), value: contextExprSchema, item: z.string(), key: contextExprSchema }),
    z.strictObject({ kind: z.literal('record'), fields: z.record(z.string(), contextExprSchema) }),
    z.strictObject({ kind: z.literal('list'), itemType: contextTypeSchema, items: z.array(contextExprSchema) }),
    z.strictObject({ kind: z.literal('template'), template: z.string(), values: z.record(z.string(), contextExprSchema) }),
    z.strictObject({ kind: z.literal('toJson'), value: contextExprSchema }),
    z.strictObject({ kind: z.literal('construct'), name: z.string(), value: contextExprSchema }),
    z.strictObject({ kind: z.literal('measure'), value: contextExprSchema, unit: z.enum(['bytes', 'items', 'media']) }),
    z.strictObject({ kind: z.literal('call'), catalog: z.enum(['projection', 'subprogram']), name: z.string(), arguments: z.record(z.string(), contextExprSchema) }),
]));
export type ContextExpr = z.output<typeof contextExprSchema>;
export const contextPredicateSchema: z.ZodType<PredicateNode, PredicateNode> = z.lazy(() => z.discriminatedUnion('kind', [
    z.strictObject({ kind: z.literal('present'), value: contextExprSchema }), z.strictObject({ kind: z.literal('eq'), left: contextExprSchema, right: contextExprSchema }), z.strictObject({ kind: z.literal('compare'), left: contextExprSchema, operator: z.enum(['lt', 'lte', 'gt', 'gte', 'ne']), right: contextExprSchema }), z.strictObject({ kind: z.literal('contains'), value: contextExprSchema, item: contextExprSchema }), z.strictObject({ kind: z.enum(['and', 'or']), items: z.array(contextPredicateSchema) }), z.strictObject({ kind: z.literal('not'), item: contextPredicateSchema }),
]));
export type ContextPredicate = z.output<typeof contextPredicateSchema>;
export const fragmentRoleSchema = z.enum(['instruction', 'data']);
export const fragmentFormatSchema = z.enum(['text', 'json', 'media', 'adkMessages']);
export type BlockNode = {
    kind: 'group';
    id: string;
    label: string;
    items: BlockNode[];
} | {
    kind: 'emit';
    id: string;
    role: z.output<typeof fragmentRoleSchema>;
    format: z.output<typeof fragmentFormatSchema>;
    value: ExprNode;
} | {
    kind: 'if';
    id: string;
    condition: PredicateNode;
    then: BlockNode[];
    else: BlockNode[];
} | {
    kind: 'forEach';
    id: string;
    value: ExprNode;
    item: string;
    items: BlockNode[];
};
export const contextBlockSchema: z.ZodType<BlockNode, BlockNode> = z.lazy(() => z.discriminatedUnion('kind', [
    z.strictObject({ kind: z.literal('group'), id: z.string(), label: z.string(), items: z.array(contextBlockSchema) }), z.strictObject({ kind: z.literal('emit'), id: z.string(), role: fragmentRoleSchema, format: fragmentFormatSchema, value: contextExprSchema }), z.strictObject({ kind: z.literal('if'), id: z.string(), condition: contextPredicateSchema, then: z.array(contextBlockSchema), else: z.array(contextBlockSchema) }), z.strictObject({ kind: z.literal('forEach'), id: z.string(), value: contextExprSchema, item: z.string(), items: z.array(contextBlockSchema) }),
]));
export type ContextBlock = z.output<typeof contextBlockSchema>;
export const contextCapabilitySchema = z.strictObject({ id: z.string(), input: contextTypeSchema, output: contextTypeSchema });
export type ContextCapability = z.output<typeof contextCapabilitySchema>;
export const contextTypesSchema = z.record(z.string(), contextTypeSchema);
const contextStrategyDefinition = z.strictObject({ version: z.number().int().nonnegative(), id: z.string(), name: z.string(), types: contextTypesSchema.optional(), requirements: contextTypesSchema, capabilities: z.array(contextCapabilitySchema), program: z.array(contextBlockSchema) });
export interface ContextStrategy extends z.output<typeof contextStrategyDefinition> {
}
export const contextStrategySchema: z.ZodType<ContextStrategy, ContextStrategy> = contextStrategyDefinition;
const contextFunctionDefinition = z.strictObject({ parameters: contextTypesSchema, output: contextTypeSchema, body: contextExprSchema });
export interface ContextFunction extends z.output<typeof contextFunctionDefinition> {
}
export const contextFunctionSchema: z.ZodType<ContextFunction, ContextFunction> = contextFunctionDefinition;
const contextLibraryDefinition = z.strictObject({ projections: z.record(z.string(), contextFunctionSchema), subprograms: z.record(z.string(), contextFunctionSchema) });
export interface ContextLibrary extends z.output<typeof contextLibraryDefinition> {
}
export const contextLibrarySchema: z.ZodType<ContextLibrary, ContextLibrary> = contextLibraryDefinition;
export const contextDiagnosticSchema = z.object({ code: z.string(), path: z.string(), message: z.string() }).catchall(jsonValueSchema);
export type ContextDiagnostic = z.output<typeof contextDiagnosticSchema>;
export const sourceFileSchema = z.object({ key: z.string(), path: z.string(), hash: z.string(), source: z.string().optional(), diagnostics: z.array(contextDiagnosticSchema) }).catchall(jsonValueSchema);
export type SourceFile = z.output<typeof sourceFileSchema>;
export const contextFileSchema = sourceFileSchema.extend({ strategy: contextStrategySchema.optional() });
export type ContextFile = z.output<typeof contextFileSchema>;
export const contextLibraryFileSchema = sourceFileSchema.extend({ library: contextLibrarySchema.optional() });
export type ContextLibraryFile = z.output<typeof contextLibraryFileSchema>;
export const contextTypesFileSchema = sourceFileSchema.extend({ types: contextTypesSchema.optional() });
export type ContextTypesFile = z.output<typeof contextTypesFileSchema>;
export const workspaceContextSchema = z.object({ instructions: z.array(z.object({ path: z.string(), content: z.string(), hash: z.string() }).catchall(jsonValueSchema)), skills: z.array(z.object({ name: z.string(), description: z.string(), path: z.string(), manualOnly: z.boolean().optional(), hash: z.string().optional() }).catchall(jsonValueSchema)), diagnostics: z.array(z.string()), loadedSkills: z.array(z.object({ name: z.string(), path: z.string(), hash: z.string(), sourceHash: z.string().optional(), truncated: z.boolean().optional() }).catchall(jsonValueSchema)).optional() }).catchall(jsonValueSchema);
export type WorkspaceContext = z.output<typeof workspaceContextSchema>;
export const readerInputSchema = z.discriminatedUnion('kind', [z.strictObject({ kind: z.literal('literal'), value: jsonValueSchema }), z.strictObject({ kind: z.literal('state'), field: z.string().min(1), pointer: z.string().optional() })]);
export type ReaderInput = z.output<typeof readerInputSchema>;
export const readerContractSchema = z.object({ id: z.string(), version: z.string(), input: contextTypeSchema, output: z.discriminatedUnion('kind', [z.strictObject({ kind: z.literal('fixed'), dataType: contextTypeSchema }), z.strictObject({ kind: z.literal('declaredJson') })]) }).catchall(jsonValueSchema);
export type ReaderContract = z.output<typeof readerContractSchema>;
export const dataScopeSchema = z.discriminatedUnion('kind', [z.strictObject({ kind: z.enum(['flow', 'bridge']), id: z.string().min(1) }), z.strictObject({ kind: z.literal('runtime') })]);
export type DataScope = z.output<typeof dataScopeSchema>;
export const resourceProducerSchema = z.strictObject({ branch: z.string().min(1), routeId: z.string().min(1), input: contextExprSchema, outputPointer: z.string().optional() });
export type ResourceProducer = z.output<typeof resourceProducerSchema>;
export const resourceBindingSchema = z.discriminatedUnion('kind', [z.strictObject({ kind: z.literal('state'), field: z.string().min(1), pointer: z.string().optional(), encoding: z.literal('adkMessages').optional() }), z.strictObject({ kind: z.literal('attachments'), slot: z.enum(['instructions', 'skills', 'files']) }), z.strictObject({ kind: z.literal('conversation'), historyField: z.string().min(1), inputField: z.string().min(1) }), z.strictObject({ kind: z.literal('attachment'), itemId: z.string().min(1), skillName: z.string().optional() }), z.strictObject({ kind: z.literal('entity'), scope: dataScopeSchema, alias: z.string().min(1), revision: z.string().optional() }), z.strictObject({ kind: z.literal('produced'), producer: resourceProducerSchema }), z.strictObject({ kind: z.literal('reader'), reader: z.string().min(1), input: readerInputSchema })]);
export type ResourceBinding = z.output<typeof resourceBindingSchema>;
export const windowPreparationSchema = z.strictObject({ alias: z.string().min(1), prepare: z.strictObject({ branch: z.string().min(1), routeId: z.string().min(1) }).optional() });
export type WindowPreparation = z.output<typeof windowPreparationSchema>;
export const frozenContextSourceSchema = z.object({ key: z.string(), hash: z.string(), source: z.string() }).catchall(jsonValueSchema);
export type FrozenContextSource = z.output<typeof frozenContextSourceSchema>;
const contextProgramDefinition = z.object({ strategy: contextStrategySchema, types: contextTypesSchema, source: z.string(), hash: z.string(), bindings: z.record(z.string(), resourceBindingSchema), library: contextLibrarySchema, librarySources: z.array(frozenContextSourceSchema).optional(), typeSources: z.array(frozenContextSourceSchema).optional(), window: windowPreparationSchema.optional() }).catchall(jsonValueSchema);
export interface ContextProgram extends z.output<typeof contextProgramDefinition> {
}
export const contextProgramSchema: z.ZodType<ContextProgram, ContextProgram> = contextProgramDefinition;
export const artifactKindSchema = z.enum(['strategy', 'library', 'bridge', 'types', 'example']);
export type ArtifactKind = z.output<typeof artifactKindSchema>;
export const artifactSelectionSchema = z.strictObject({ kind: artifactKindSchema, key: z.string() });
export type ArtifactSelection = z.output<typeof artifactSelectionSchema>;
export const sourceArtifactSchema = artifactSelectionSchema.extend({ source: z.string(), hash: z.string() });
export type SourceArtifact = z.output<typeof sourceArtifactSchema>;
export const contextPackageSchema = z.strictObject({ version: z.number().int().nonnegative(), artifacts: z.array(sourceArtifactSchema) });
export type ContextPackage = z.output<typeof contextPackageSchema>;
export const typeExampleSchema = z.object({ version: z.number().int(), id: z.string(), label: z.string(), schemaHash: z.string(), dataType: contextTypeSchema, types: contextTypesSchema, value: jsonValueSchema }).catchall(jsonValueSchema);
export type TypeExample = z.output<typeof typeExampleSchema>;
export const sourceTypeEntrySchema = z.object({ id: z.string(), label: z.string(), category: z.string(), type: contextTypeSchema, types: contextTypesSchema, origin: z.string(), providers: z.array(z.string()) }).catchall(jsonValueSchema);
export type SourceTypeEntry = z.output<typeof sourceTypeEntrySchema>;
