import { z } from 'zod';
import { jsonValueSchema, jsonObjectSchema } from '../core/json.js';
/** Historical selections retain provider-specific options, including old thinking budgets. */
export const historicalModelSelectionSchema = z.object({ provider: z.string(), model: z.string(), reasoningEffort: z.string().nullish(), reasoningSummary: z.string().nullish(), textVerbosity: z.string().nullish(), thinkingBudget: z.number().nullish() }).catchall(jsonValueSchema);
export type HistoricalModelSelection = z.output<typeof historicalModelSelectionSchema>;
export const modelEntrySchema = z.object({ provider: z.string(), id: z.string(), label: z.string(), reasoningLevels: z.array(z.string()), reasoningDefault: z.string().nullish(), thinkingBudget: z.boolean().optional() }).catchall(jsonValueSchema);
export type ModelEntry = z.output<typeof modelEntrySchema>;
export const modelBindingSchema = z.enum(['fixed', 'runtime']);
export type ModelBinding = z.output<typeof modelBindingSchema>;
export const generateContentConfigSchema = z.strictObject({ temperature: z.number().nullish(), top_p: z.number().nullish(), top_k: z.number().int().min(-2147483648).max(2147483647).nullish(), frequency_penalty: z.number().nullish(), presence_penalty: z.number().nullish(), max_output_tokens: z.number().int().min(-2147483648).max(2147483647).nullish(), seed: z.number().int().nullish(), top_logprobs: z.number().int().min(0).max(255).nullish(), stop_sequences: z.array(z.string()).optional(), response_schema: jsonValueSchema.optional(), cached_content: z.string().nullish(), extensions: jsonObjectSchema.optional() });
export type GenerateContentConfig = z.output<typeof generateContentConfigSchema>;
export const adkInlineDataSchema = z.object({ mime_type: z.string(), data: z.array(z.number().int().min(0).max(255)), uri: z.string().nullish(), annotations: jsonValueSchema.optional() }).catchall(jsonValueSchema);
export type AdkInlineData = z.output<typeof adkInlineDataSchema>;
export const adkFileDataSchema = z.object({ mime_type: z.string(), file_uri: z.string(), annotations: jsonValueSchema.optional() }).catchall(jsonValueSchema);
export type AdkFileData = z.output<typeof adkFileDataSchema>;
export const adkFunctionResponseSchema = z.object({ name: z.string(), response: jsonValueSchema, inline_data: z.array(adkInlineDataSchema).optional(), file_data: z.array(adkFileDataSchema).optional() }).catchall(jsonValueSchema);
export type AdkFunctionResponse = z.output<typeof adkFunctionResponseSchema>;
export const adkPartSchema = z.union([z.object({ thinking: z.string(), signature: z.string().nullish() }).catchall(jsonValueSchema), z.object({ text: z.string() }).catchall(jsonValueSchema), adkInlineDataSchema, adkFileDataSchema, z.object({ name: z.string(), args: jsonValueSchema, id: z.string().nullish(), thought_signature: z.string().nullish() }).catchall(jsonValueSchema), z.object({ functionResponse: adkFunctionResponseSchema, id: z.string().nullish(), annotations: jsonValueSchema.optional() }).catchall(jsonValueSchema), z.object({ server_tool_call: jsonValueSchema }).catchall(jsonValueSchema), z.object({ server_tool_response: jsonValueSchema }).catchall(jsonValueSchema), z.object({ resource: z.union([z.object({ uri: z.string(), mime_type: z.string().nullish(), text: z.string() }).catchall(jsonValueSchema), z.object({ uri: z.string(), mime_type: z.string().nullish(), data: z.array(z.number().int().min(0).max(255)) }).catchall(jsonValueSchema)]) }).catchall(jsonValueSchema)]);
export type AdkPart = z.output<typeof adkPartSchema>;
export const adkContentSchema = z.object({ role: z.string(), parts: z.array(adkPartSchema) }).catchall(jsonValueSchema);
export type AdkContent = z.output<typeof adkContentSchema>;
export const toolDeclarationSchema = z.object({ name: z.string().optional(), description: z.string().optional(), parameters: jsonValueSchema.optional() }).catchall(jsonValueSchema);
export type ToolDeclaration = z.output<typeof toolDeclarationSchema>;
const modelRequestDefinition = z.object({ model: z.string(), contents: z.array(adkContentSchema), config: generateContentConfigSchema.nullish(), tools: z.record(z.string(), toolDeclarationSchema), previous_response_id: z.string().nullish() }).catchall(jsonValueSchema);
export interface ModelRequest extends z.output<typeof modelRequestDefinition> {
}
export const modelRequestSchema: z.ZodType<ModelRequest, ModelRequest> = modelRequestDefinition;
export const tokenUsageSchema = z.object({ prompt_token_count: z.number().int(), candidates_token_count: z.number().int(), total_token_count: z.number().int(), cache_read_input_token_count: z.number().int().nullish(), cache_creation_input_token_count: z.number().int().nullish(), thinking_token_count: z.number().int().nullish(), audio_input_token_count: z.number().int().nullish(), audio_output_token_count: z.number().int().nullish(), cost: z.number().nullish(), is_byok: z.boolean().nullish(), provider_usage: jsonValueSchema.optional() }).catchall(jsonValueSchema);
export type TokenUsage = z.output<typeof tokenUsageSchema>;
export const modelResponseSchema = z.object({ provider: z.string(), model: z.string(), preparationId: z.string().optional(), contextNode: z.string().nullish(), contextProgramHash: z.string().optional(), contextProgramVersion: z.number().int().optional(), invocationId: z.string().optional(), requestRef: z.string().optional(), nodePath: z.string().optional(), usage: tokenUsageSchema.optional(), providerMetadata: jsonValueSchema.optional(), finishReason: z.string().optional(), reasoningSummary: z.string().optional(), contextSnapshotId: z.string().optional(), selection: historicalModelSelectionSchema.optional(), runtimeSelection: historicalModelSelectionSchema.optional() }).catchall(jsonValueSchema);
export type ModelResponse = z.output<typeof modelResponseSchema>;
export const toolCallSchema = z.object({ name: z.string(), args: jsonObjectSchema, id: z.string().nullish() }).catchall(jsonValueSchema);
export type ToolCall = z.output<typeof toolCallSchema>;
export const toolResultSchema = z.object({ id: z.string().nullish(), name: z.string(), result: jsonValueSchema }).catchall(jsonValueSchema);
export type ToolResult = z.output<typeof toolResultSchema>;
