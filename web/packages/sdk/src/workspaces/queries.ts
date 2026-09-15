import { z } from 'zod';

export const browseDirectorySchema = z.strictObject({
  path: z.string().optional(), showHidden: z.boolean().optional(),
});
export type BrowseDirectoryQuery = z.input<typeof browseDirectorySchema>;

/** Commands on scoped resources pass this explicitly; the server also supports its default workspace. */
export const workspaceQuerySchema = z.strictObject({ workspaceId: z.string().min(1).optional() });
export type WorkspaceQuery = z.input<typeof workspaceQuerySchema>;
