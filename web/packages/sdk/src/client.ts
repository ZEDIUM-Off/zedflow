import { createSessionsClient, type SessionsClient } from './sessions/client.js';
import { createGenerationClient, type GenerationClient } from './generation/client.js';
import { createRunsClient, type RunsClient } from './runs/client.js';
import { createCompositionClient, type CompositionClient } from './composition/client.js';
import { createFlowsClient, type FlowsClient } from './flows/client.js';
import { createContextClient, type ContextClient } from './context/client.js';
import { FetchTransport, type FetchTransportOptions } from './core/fetch.js';
import type { Transport } from './core/transport.js';
import { createWorkspacesClient, type WorkspacesClient } from './workspaces/client.js';
import { createDaemonClient, type DaemonClient } from './daemon/client.js';
import { createModelsClient, type ModelsClient } from './models/client.js';
export type ClientOptions = FetchTransportOptions | {
    transport: Transport;
};
/** Domain clients share one configured transport, never a process-global client. */
export class ZedflowClient {
    readonly context: ContextClient;
    readonly flows: FlowsClient;
    readonly composition: CompositionClient;
    readonly runs: RunsClient;
    readonly generation: GenerationClient;
    readonly sessions: SessionsClient;
    readonly transport: Transport;
    readonly workspaces: WorkspacesClient;
    readonly models: ModelsClient;
    readonly daemon: DaemonClient;
    constructor(options: ClientOptions) {
        this.transport = 'transport' in options ? options.transport : new FetchTransport(options);
        this.context = createContextClient(this.transport);
        this.flows = createFlowsClient(this.transport);
        this.composition = createCompositionClient(this.transport);
        this.runs = createRunsClient(this.transport);
        this.generation = createGenerationClient(this.transport);
        this.sessions = createSessionsClient(this.transport);
        this.workspaces = createWorkspacesClient(this.transport);
        this.models = createModelsClient(this.transport);
        this.daemon = createDaemonClient(this.transport);
    }
}
export function createClient(options: ClientOptions): ZedflowClient {
    return new ZedflowClient(options);
}
