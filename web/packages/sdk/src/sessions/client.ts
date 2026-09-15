import type { Transport } from '../core/transport.js';
import { validateInput } from '../core/schema.js';
import { exportSessionsSchema, importSessionsSchema, type ExportSessionsInput, type ImportSessionsInput } from './commands.js';
import { sessionExportResponseSchema, sessionImportResponseSchema, type SessionExportResponse, type SessionImportResponse } from './results.js';
import { sessionDownloadLinkSchema } from './queries.js';

export function createSessionsClient(transport: Transport) {
  return {
    async export(input: ExportSessionsInput, signal?: AbortSignal): Promise<SessionExportResponse> {
      const body = validateInput('sessions.export', exportSessionsSchema, input);
      return transport.json({ operation: 'sessions.export', method: 'POST', path: 'sessions/export', body, ...(signal ? { signal } : {}) }, sessionExportResponseSchema);
    },
    async import(input: ImportSessionsInput, signal?: AbortSignal): Promise<SessionImportResponse> {
      const body = validateInput('sessions.import', importSessionsSchema, input);
      return transport.json({ operation: 'sessions.import', method: 'POST', path: 'sessions/import', body, ...(signal ? { signal } : {}) }, sessionImportResponseSchema);
    },
    /** Resolve the returned export link against this client's configured API base. */
    async downloadExport(downloadUrl: string, signal?: AbortSignal): Promise<Uint8Array> {
      const query = validateInput('sessions.downloadExport', sessionDownloadLinkSchema, downloadUrl);
      return transport.bytes({ operation: 'sessions.downloadExport', method: 'GET', path: `sessions/exports/${query.archiveId}.zip`, query: { workspaceId: query.workspaceId }, ...(signal ? { signal } : {}) });
    },
  };
}
export type SessionsClient = ReturnType<typeof createSessionsClient>;
