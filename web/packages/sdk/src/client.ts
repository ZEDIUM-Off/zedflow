import { FetchTransport, type FetchTransportOptions } from './core/fetch.js';
import type { Transport } from './core/transport.js';

export type ClientOptions = FetchTransportOptions | { transport: Transport };

/** Domain clients share one configured transport, never a process-global client. */
export class ZedflowClient {
  readonly transport: Transport;
  constructor(options: ClientOptions) {
    this.transport = 'transport' in options ? options.transport : new FetchTransport(options);
  }
}

export function createClient(options: ClientOptions): ZedflowClient {
  return new ZedflowClient(options);
}
