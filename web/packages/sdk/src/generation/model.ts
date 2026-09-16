import { z } from 'zod';
import { jsonValueSchema } from '../core/json.js';

export const generatedFileSchema = z.object({ path: z.string(), content: z.string(), encoding: z.enum(['utf8', 'base64']).optional() }).catchall(jsonValueSchema);
export type GeneratedFile = z.output<typeof generatedFileSchema>;

export const executionRevisionSchema = z.object({
  runId: z.string(),
  nodePath: z.string(),
  occurrenceId: z.string().nullable(),
  hash: z.string(),
  graphRef: z.string().nullable(),
}).catchall(jsonValueSchema);
export type ExecutionRevision = z.output<typeof executionRevisionSchema>;

/** Decode exactly the bytes supplied by the daemon, including package assets. */
export function generatedFileBytes(file: GeneratedFile): Uint8Array<ArrayBuffer> {
  if (file.encoding !== 'base64') return new TextEncoder().encode(file.content);
  const decoded = atob(file.content);
  return Uint8Array.from(decoded, character => character.charCodeAt(0));
}
