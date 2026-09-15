import { z } from 'zod';
import { workspaceSchema } from './model.js';

export const workspaceListSchema = z.array(workspaceSchema);
export type WorkspaceList = z.output<typeof workspaceListSchema>;
