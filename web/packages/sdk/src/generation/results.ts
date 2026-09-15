import { z } from 'zod';
import { jsonValueSchema } from '../core/json.js';
import { executionRevisionSchema, generatedFileSchema } from './model.js';

export const generatedSourceSchema = z.object({
  files: z.array(generatedFileSchema),
  executionRevision: executionRevisionSchema.optional(),
}).catchall(jsonValueSchema);
export type GeneratedSource = z.output<typeof generatedSourceSchema>;

export const graphValidationSchema = z.object({ valid: z.literal(true), adk: z.string() }).catchall(jsonValueSchema);
export type GraphValidation = z.output<typeof graphValidationSchema>;

export const buildResultSchema = z.object({
  success: z.boolean(),
  output: z.string(),
  directory: z.string(),
  files: z.array(generatedFileSchema),
}).catchall(jsonValueSchema);
export type BuildResult = z.output<typeof buildResultSchema>;
