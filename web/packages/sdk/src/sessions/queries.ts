import { z } from 'zod';

export const sessionDownloadQuerySchema = z.strictObject({
  workspaceId: z.string().min(1),
  archiveId: z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i),
});
export type SessionDownloadQuery = z.input<typeof sessionDownloadQuerySchema>;

/** Only the daemon's relative export-link format is accepted, never an arbitrary URL. */
export const sessionDownloadLinkSchema = z.string().transform((value, context) => {
  const match = /^\/api\/sessions\/exports\/([^/?#]+)\.zip\?([^#]+)$/.exec(value);
  if (match) {
    const query = new URLSearchParams(match[2]);
    const entries = [...query];
    if (entries.length === 1 && entries[0]?.[0] === 'workspaceId') {
      const parsed = sessionDownloadQuerySchema.safeParse({ archiveId: match[1], workspaceId: entries[0][1] });
      if (parsed.success) return parsed.data;
    }
  }
  context.addIssue({ code: 'custom', message: 'Expected a session export link with one explicit workspaceId' });
  return z.NEVER;
});
