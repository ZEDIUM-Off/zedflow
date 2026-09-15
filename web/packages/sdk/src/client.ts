import { FetchTransport, type FetchTransportOptions } from './core/fetch.js';
import type { Transport } from './core/transport.js';
import { createWorkspacesClient, type WorkspacesClient } from './workspaces/client.js';

export type ClientOptions = FetchTransportOptions | { transport: Transport };

/** Domain clients share one configured transport, never a process-global client. */
export class ZedflowClient {
  readonly transport: Transport;
  readonly workspaces: WorkspacesClient;
  constructor(options: ClientOptions) {
    this.transport = 'transport' in options ? options.transport : new FetchTransport(options);
    this.workspaces = createWorkspacesClient(this.transport);
  }
}

export function createClient(options: ClientOptions): ZedflowClient {
  return new ZedflowClient(options);
}
