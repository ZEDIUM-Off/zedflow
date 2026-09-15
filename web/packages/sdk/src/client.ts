import { RunDetails } from './runs/details.js';
import { ExecutedDefinitions } from './runs/definitions.js';
import { followRun, type FollowRunOptions, type RunFollower } from './runs/sync.js';
import { createHttpRunTransport } from './transports/http.js';
import { createDaemonConnection, type DaemonConnectionOptions, type DaemonConnection } from './daemon/connection.js';
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
    readonly details: RunDetails;
    readonly definitions: ExecutedDefinitions;
    private readonly disposables = new Set<() => void>();
    private disposed = false;
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
        this.details = new RunDetails(this.runs);
        this.definitions = new ExecutedDefinitions(this.runs);
        this.generation = createGenerationClient(this.transport);
        this.sessions = createSessionsClient(this.transport);
        this.workspaces = createWorkspacesClient(this.transport);
        this.models = createModelsClient(this.transport);
        this.daemon = createDaemonClient(this.transport);
    }
    followRun(options: Omit<FollowRunOptions, 'transport'>): RunFollower {
        if (this.disposed) throw new Error('Client disposed');
        const follower = followRun({ ...options, transport: createHttpRunTransport(this.runs) });
        const close = () => { follower.close(); this.disposables.delete(close); };
        this.disposables.add(close);
        return { get state() { return follower.state; }, refresh: follower.refresh, prependTimeline: follower.prependTimeline, close };
    }
    connectDaemon(options: Omit<DaemonConnectionOptions, 'daemon'> = {}): DaemonConnection {
        if (this.disposed) throw new Error('Client disposed');
        const connection = createDaemonConnection({ ...options, daemon: this.daemon });
        const dispose = () => { connection.dispose(); this.disposables.delete(dispose); };
        this.disposables.add(dispose);
        return { get state() { return connection.state; }, seen: connection.seen, check: connection.check, dispose };
    }
    dispose(): void {
        if (this.disposed) return;
        this.disposed = true;
        for (const close of this.disposables) close();
        this.details.dispose(); this.definitions.dispose();
    }
}
export function createClient(options: ClientOptions): ZedflowClient {
    return new ZedflowClient(options);
}
