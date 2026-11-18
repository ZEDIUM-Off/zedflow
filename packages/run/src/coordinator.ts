import type {
  Engine,
  Event,
  Json,
  Loader,
  RecordEntry,
  RecordStore,
  RunService,
  ReloadOptions,
  RunUpdate,
} from '@zf/core';
import { CandidateLoader } from './candidate-loader.ts';
import { RunExecution } from './run-execution.ts';
import { RunLifecycle } from './run-lifecycle.ts';
import { RunNotifications } from './run-notifications.ts';

export interface RunCoordinatorOptions {
  readonly loader: Loader;
  readonly engine: Engine;
  readonly store: RecordStore;
}

export class RunCoordinator implements RunService {
  readonly #loader: CandidateLoader;
  readonly #engine: Engine;
  readonly #store: RecordStore;
  readonly #runs = new Map<string, RunLifecycle>();
  readonly #notifications: RunNotifications;

  public constructor(options: RunCoordinatorOptions) {
    this.#loader = new CandidateLoader(options.loader);
    this.#engine = options.engine;
    this.#store = options.store;
    this.#notifications = new RunNotifications((runId) => this.#get(runId).update());
  }

  public async launch(path: string, origin?: string): Promise<string> {
    if (origin !== undefined) {
      this.#get(origin);
    }
    const source = await this.#loader.load(path);
    const execution = new RunExecution(source, this.#engine, this.#store, (runId) =>
      this.#notifications.notify(runId),
    );
    execution.open(origin);
    this.#runs.set(execution.id, new RunLifecycle(execution, this.#loader, this.#notifications));
    execution.start();
    return execution.id;
  }

  public send(runId: string, event: Event): void {
    this.#get(runId).send(event);
  }

  public view(runId: string): Json {
    return this.#get(runId).view();
  }

  public records(runId: string): RecordEntry[] {
    return this.#store.read(runId);
  }

  public subscribe(runId: string, listener: (update: RunUpdate) => void): () => void {
    this.#get(runId);
    return this.#notifications.subscribe(runId, listener);
  }

  public reload(runId: string, options: ReloadOptions = {}): Promise<void> {
    try {
      return this.#get(runId).reload(options);
    } catch (error) {
      return Promise.reject(error);
    }
  }

  public stop(runId: string): void {
    this.#get(runId).stop();
  }

  #get(runId: string): RunLifecycle {
    const run = this.#runs.get(runId);
    if (!run) {
      throw new Error('Unknown run');
    }
    return run;
  }
}
