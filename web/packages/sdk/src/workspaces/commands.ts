import { z } from 'zod';

export const openWorkspaceSchema = z.strictObject({ path: z.string().min(1) });
export type OpenWorkspaceInput = z.input<typeof openWorkspaceSchema>;

export const updateWorkspaceSchema = z.strictObject({
  name: z.string().refine(value => value.trim().length > 0, 'Workspace name must not be blank').optional(),
  open: z.boolean().optional(),
});
export type UpdateWorkspaceInput = z.input<typeof updateWorkspaceSchema>;
