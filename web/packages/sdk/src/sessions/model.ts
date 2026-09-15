import { z } from 'zod';
import { jsonValueSchema } from '../core/json.js';

export const exportedSessionSchema = z.object({
  sessionId: z.string(),
  path: z.string(),
  archiveHash: z.string(),
}).catchall(jsonValueSchema);
export type ExportedSession = z.output<typeof exportedSessionSchema>;

/** Import provenance is distinct from the current workspace binding. */
export const sessionImportInfoSchema = z.object({
  archiveHash: z.string(),
  sourceSessionId: z.string(),
  sourceWorkspace: z.object({ id: z.string().nullish(), path: z.string().nullable() }).catchall(jsonValueSchema),
  importedAt: z.number(),
  resumeBlocked: z.array(z.string()),
}).catchall(jsonValueSchema);
export type SessionImportInfo = z.output<typeof sessionImportInfoSchema>;
