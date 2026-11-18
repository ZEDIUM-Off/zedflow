import type { Event, Json, MachineSource, ReloadOptions, ReloadStatus, RunUpdate } from '@zf/core';
import type { CandidateLoader } from './candidate-loader.ts';
import type { RunExecution } from './run-execution.ts';
import type { RunNotifications } from './run-notifications.ts';

const requireAvailable = (busy: boolean, stopped: boolean): void => {
  if (busy || stopped) {
    throw new Error('Run unavailable');
  }
};

// Owns command availability and the exclusive reload request, including its waiting interval.
export class RunLifecycle {
  readonly #execution: RunExecution;
  readonly #loader: CandidateLoader;
  readonly #notifications: RunNotifications;
  #busy = false;
  #stopped = false;
  #reload: ReloadStatus | undefined;
  #pendingReload = false;

  public constructor(
    execution: RunExecution,
    loader: CandidateLoader,
    notifications: RunNotifications,
  ) {
    this.#execution = execution;
    this.#loader = loader;
    this.#notifications = notifications;
  }

  public send(event: Event): void {
    requireAvailable(this.#busy, this.#stopped);
    this.#execution.send(event);
  }

  public view(): Json {
    return this.#execution.view();
  }

  public update(): RunUpdate {
    let view: Json;
    try {
      view = this.#execution.view();
    } catch {
      view = { error: 'View is not JSON-serializable' };
    }
    return {
      view,
      generation: this.#execution.generation,
      stopped: this.#stopped,
      ...(this.#reload === undefined ? {} : { reload: this.#reload }),
    };
  }

  public stop(): void {
    if (this.#busy) {
      throw new Error('Run unavailable');
    }
    if (this.#stopped) {
      return;
    }
    this.#execution.stop(() => {
      this.#stopped = true;
    });
  }

  public async reload(options: ReloadOptions): Promise<void> {
    this.#claimReload();
    try {
      options.signal?.throwIfAborted();
      while (options.wait && this.#execution.checkpointStatus() === 'busy') {
        this.#reload = 'waiting';
        this.#notifications.notify(this.#execution.id);
        await this.#notifications.wait(
          this.#execution.id,
          () => this.#checkpointReady(),
          options.signal,
        );
      }
      const source = await this.#prepare(options.signal);
      this.#activateCandidate(source, options.signal);
    } catch (error) {
      this.#reload = 'refused';
      throw error;
    } finally {
      this.#finishReload();
    }
  }

  #prepare(signal?: AbortSignal): Promise<MachineSource> {
    requireAvailable(this.#busy, this.#stopped);
    signal?.throwIfAborted();
    this.#busy = true;
    this.#execution.checkpoint();
    this.#reload = 'preparing';
    this.#notifications.notify(this.#execution.id);
    return this.#loader.load(this.#execution.path, signal);
  }

  #checkpointReady(): boolean {
    requireAvailable(this.#busy, this.#stopped);
    if (this.#execution.checkpointStatus() === 'busy') {
      return false;
    }
    this.#execution.checkpoint();
    return true;
  }

  #claimReload(): void {
    requireAvailable(this.#busy, this.#stopped);
    if (this.#pendingReload) {
      throw new Error('Run unavailable');
    }
    this.#pendingReload = true;
  }

  #activateCandidate(source: MachineSource, signal?: AbortSignal): void {
    signal?.throwIfAborted();
    this.#execution.replace(source);
    this.#reload = 'applied';
  }

  #finishReload(): void {
    this.#busy = false;
    this.#pendingReload = false;
    this.#notifications.notify(this.#execution.id);
  }
}
