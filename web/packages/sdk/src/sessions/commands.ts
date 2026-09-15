import { z } from 'zod';

export const exportSessionsSchema = z.strictObject({
  workspaceId: z.string().min(1),
  sessionIds: z.array(z.string().min(1)).min(1).max(100)
    .refine(ids => new Set(ids).size === ids.length, 'Select each session only once'),
});
export type ExportSessionsInput = z.input<typeof exportSessionsSchema>;

/** Path is interpreted by the daemon relative to the selected workspace. */
export const importSessionsSchema = z.strictObject({
  workspaceId: z.string().min(1),
  path: z.string().min(1),
});
export type ImportSessionsInput = z.input<typeof importSessionsSchema>;
