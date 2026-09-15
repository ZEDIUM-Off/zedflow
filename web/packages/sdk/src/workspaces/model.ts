import { z } from 'zod';
import { jsonValueSchema } from '../core/json.js';

export const workspaceSchema = z.object({
  id: z.string(), name: z.string(), path: z.string(), open: z.boolean(),
}).catchall(jsonValueSchema);
export type Workspace = z.output<typeof workspaceSchema>;

export const directoryListingSchema = z.object({
  path: z.string(), parent: z.string().nullable(), home: z.string(),
  entries: z.array(z.object({ name: z.string(), path: z.string(), directory: z.literal(true) }).catchall(jsonValueSchema)),
  diagnostics: z.array(z.string()).optional(),
}).catchall(jsonValueSchema);
export type DirectoryListing = z.output<typeof directoryListingSchema>;
