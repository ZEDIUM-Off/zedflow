import type { Event, RunService, RunUpdate } from '@zf/core';
import { isAgentActive, readAgentSnapshot } from './agent-view.ts';
import { errorText } from './terminal-notices.ts';
import type { TerminalNotices } from './terminal-notices.ts';
import type { TerminalOperationScope } from './terminal-operation-scope.ts';
import { TerminalReload } from './terminal-reload.ts';
import type { TerminalView } from './terminal-view.ts';

/** Owns the subscription and latest snapshot for a single native run. */
export class TerminalRunLink {
  readonly #runs: RunService;
  readonly #runId: string;
  readonly #view: TerminalView;
  readonly #notices: TerminalNotices;
  readonly #reload: TerminalReload;
  #snapshot = readAgentSnapshot(null);
  #unsubscribe: (() => void) | undefined;
  #closed = false;

  public constructor(
    runs: RunService,
    runId: string,
    view: TerminalView,
    notices: TerminalNotices,
  ) {
    this.#runs = runs;
    this.#runId = runId;
    this.#view = view;
    this.#notices = notices;
    this.#reload = new TerminalReload(runs, runId, notices, view.chrome);
  }

  public get agent(): boolean {
    return this.#view.agent;
  }

  public get active(): boolean {
    return this.agent && isAgentActive(this.#snapshot);
  }

  public subscribe(onStopped: () => void, acceptingUpdates: () => boolean): void {
    const unsubscribe = this.#runs.subscribe(this.#runId, (update) => {
      // Closing suppresses synchronous updates caused by abort, before unsubscription.
      if (acceptingUpdates()) {
        this.#onUpdate(update, onStopped);
      }
    });
    // A synchronous stopped update can close the UI before subscribe returns.
    if (this.#closed) {
      unsubscribe();
    } else {
      this.#unsubscribe = unsubscribe;
    }
  }

  public close(): void {
    this.#closed = true;
    this.#unsubscribe?.();
    this.#unsubscribe = undefined;
  }

  public send(event: Event): boolean {
    try {
      this.#runs.send(this.#runId, event);
      return true;
    } catch (error) {
      this.#notices.add(`Could not send ${event.type}: ${errorText(error)}`);
      return false;
    }
  }

  public reload(scope: TerminalOperationScope): Promise<void> {
    return this.#reload.run(scope);
  }

  public reset(): boolean {
    if (!this.send({ type: 'RESET' })) {
      return false;
    }
    this.#view.reset();
    return true;
  }

  #onUpdate(update: RunUpdate, onStopped: () => void): void {
    if (this.#closed) {
      return;
    }
    this.#snapshot = readAgentSnapshot(update.view);
    this.#view.update(update, this.#snapshot);
    if (update.stopped) {
      this.#notices.add('The run stopped.');
      onStopped();
    }
  }
}
