import { z } from 'zod';
import type { Transport } from '../core/transport.js';
import { validateInput } from '../core/schema.js';
import { workspaceSchema, directoryListingSchema, type Workspace, type DirectoryListing } from './model.js';
import { workspaceListSchema, type WorkspaceList } from './results.js';
import { browseDirectorySchema, type BrowseDirectoryQuery } from './queries.js';
import { openWorkspaceSchema, updateWorkspaceSchema, type OpenWorkspaceInput, type UpdateWorkspaceInput } from './commands.js';

export function createWorkspacesClient(transport: Transport) {
  return {
    list(signal?: AbortSignal): Promise<WorkspaceList> {
      return transport.json({ operation: 'workspaces.list', method: 'GET', path: 'workspaces', ...(signal ? { signal } : {}) }, workspaceListSchema);
    },
    async browse(input: BrowseDirectoryQuery = {}, signal?: AbortSignal): Promise<DirectoryListing> {
      const query = validateInput('workspaces.browse', browseDirectorySchema, input);
      return transport.json({ operation: 'workspaces.browse', method: 'GET', path: 'filesystem', query, ...(signal ? { signal } : {}) }, directoryListingSchema);
    },
    async open(input: OpenWorkspaceInput, signal?: AbortSignal): Promise<Workspace> {
      const body = validateInput('workspaces.open', openWorkspaceSchema, input);
      return transport.json({ operation: 'workspaces.open', method: 'POST', path: 'workspaces', body, ...(signal ? { signal } : {}) }, workspaceSchema);
    },
    async update(id: string, input: UpdateWorkspaceInput, signal?: AbortSignal): Promise<Workspace> {
      const key = validateInput('workspaces.update', z.string().min(1), id);
      const parsed = validateInput('workspaces.update', updateWorkspaceSchema, input);
      const body = {
        ...(parsed.name !== undefined ? { name: parsed.name } : {}),
        ...(parsed.open !== undefined ? { open: parsed.open } : {}),
      };
      return transport.json({ operation: 'workspaces.update', method: 'PATCH', path: `workspaces/${encodeURIComponent(key)}`, body, ...(signal ? { signal } : {}) }, workspaceSchema);
    },
  };
}
export type WorkspacesClient = ReturnType<typeof createWorkspacesClient>;
