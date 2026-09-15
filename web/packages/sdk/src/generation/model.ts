import { z } from 'zod';
import { jsonValueSchema } from '../core/json.js';

export const generatedFileSchema = z.object({ path: z.string(), content: z.string() }).catchall(jsonValueSchema);
export type GeneratedFile = z.output<typeof generatedFileSchema>;

export const executionRevisionSchema = z.object({
  runId: z.string(),
  nodePath: z.string(),
  occurrenceId: z.string().nullable(),
  hash: z.string(),
  graphRef: z.string().nullable(),
}).catchall(jsonValueSchema);
export type ExecutionRevision = z.output<typeof executionRevisionSchema>;
