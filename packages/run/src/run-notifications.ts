import type { RunUpdate } from '@zf/core';
import { CheckpointWait } from './checkpoint-wait.ts';

export class RunNotifications {
  readonly #listeners = new Map<string, Set<(update: RunUpdate) => void>>();
  readonly #waiters = new Map<string, Set<() => void>>();
  readonly #notifications = new Set<string>();
  readonly #update: (runId: string) => RunUpdate;

  public constructor(update: (runId: string) => RunUpdate) {
    this.#update = update;
  }

  public subscribe(runId: string, listener: (update: RunUpdate) => void): () => void {
    const listeners = this.#listeners.get(runId) ?? new Set();
    this.#listeners.set(runId, listeners);
    listeners.add(listener);
    try {
      listener(this.#update(runId));
    } catch (error) {
      listeners.delete(listener);
      throw error;
    }
    return () => {
      listeners.delete(listener);
      if (listeners.size === 0) {
        this.#listeners.delete(runId);
      }
    };
  }

  public wait(runId: string, ready: () => boolean, signal?: AbortSignal): Promise<void> {
    const waiters = this.#waiters.get(runId) ?? new Set();
    this.#waiters.set(runId, waiters);
    return new CheckpointWait(waiters, ready, () => this.#waiters.delete(runId), signal).wait();
  }

  public notify(runId: string): void {
    if (this.#notifications.has(runId)) {
      return;
    }
    this.#notifications.add(runId);
    // Observe after the native macrostep; coalesce delivery, not execution. Wake before observers.
    queueMicrotask(() => {
      this.#notifications.delete(runId);
      for (const wake of this.#waiters.get(runId) ?? []) {
        wake();
      }
      this.#deliver(runId);
    });
  }

  #deliver(runId: string): void {
    const listeners = this.#listeners.get(runId);
    if (!listeners?.size) {
      return;
    }
    const update = this.#update(runId);
    for (const listener of listeners) {
      try {
        listener(structuredClone(update));
      } catch {
        listeners.delete(listener);
      } // Faulty observers cannot stop the native actor.
    }
  }
}
