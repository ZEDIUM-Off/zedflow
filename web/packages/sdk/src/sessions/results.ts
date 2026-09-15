import { z } from 'zod';
import { jsonValueSchema } from '../core/json.js';
import { exportedSessionSchema } from './model.js';
import { runSchema } from '../runs/model.js';

export const sessionExportResponseSchema = z.object({
  exports: z.array(exportedSessionSchema),
  downloadUrl: z.string(),
}).catchall(jsonValueSchema);
export type SessionExportResponse = z.output<typeof sessionExportResponseSchema>;

const sessionImportResponseDefinition = z.object({
  runs: z.array(runSchema),
  imported: z.number().int().nonnegative(),
  unchanged: z.number().int().nonnegative(),
}).catchall(jsonValueSchema);
export interface SessionImportResponse extends z.output<typeof sessionImportResponseDefinition> {}
export const sessionImportResponseSchema: z.ZodType<SessionImportResponse, SessionImportResponse> = sessionImportResponseDefinition;
