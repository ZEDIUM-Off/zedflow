import { z } from 'zod';
export const applyUpdateSchema = z.strictObject({
    releaseId: z.string().regex(/^[a-f0-9]{64}$/),
    expectedDaemonBuildId: z.string().min(1),
});
export type ApplyUpdateInput = z.input<typeof applyUpdateSchema>;
