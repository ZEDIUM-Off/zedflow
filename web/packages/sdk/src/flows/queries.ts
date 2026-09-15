export { workspaceQuerySchema, type WorkspaceQuery } from '../workspaces/queries.js';
import { z } from 'zod';
export const deleteFlowQuerySchema = z.strictObject({ workspaceId: z.string().min(1).optional(), expectedHash: z.string().min(1) });
export type DeleteFlowQuery = z.input<typeof deleteFlowQuerySchema>;
