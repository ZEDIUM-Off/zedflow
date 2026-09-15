import { z } from 'zod';
import { compositionSchema } from '../flows/model.js';

export const generateFlowSchema = z.strictObject({
  workspaceId: z.string().min(1).optional(),
  flowKey: z.string().min(1),
  flowHash: z.string().min(1),
});
export const generatePassageSchema = z.strictObject({
  workspaceId: z.string().min(1).optional(),
  runId: z.string().min(1),
  nodePath: z.string().min(1).optional(),
  occurrenceId: z.string().min(1).optional(),
  hash: z.string().min(1).optional(),
});
export const generateDraftSchema = z.strictObject({
  workspaceId: z.string().min(1).optional(),
  composition: compositionSchema,
});
export const generateSchema = z.union([generateFlowSchema, generatePassageSchema, generateDraftSchema, compositionSchema]);
export type GenerateInput = z.input<typeof generateSchema>;
