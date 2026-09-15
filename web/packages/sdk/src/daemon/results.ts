import { z } from 'zod';
import { jsonValueSchema } from '../core/json.js';
export const updateAcknowledgementSchema = z.object({
    id: z.string(), target: z.string(), source: z.string().nullable(),
    expectedDaemonBuildId: z.string(), createdAt: z.number(),
}).catchall(jsonValueSchema);
export type UpdateAcknowledgement = z.output<typeof updateAcknowledgementSchema>;
