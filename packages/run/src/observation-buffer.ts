import { json } from '@zf/core';
import type { Frame, RecordStore } from '@zf/core';

// Buffer observations from a candidate until its definition and checkpoint validate.
export class ObservationBuffer {
  readonly #store: RecordStore;
  readonly #runId: string;
  readonly #generation: number;
  readonly #pending: Frame[] = [];
  readonly #notify: () => void;
  #committed = false;

  public constructor(options: {
    store: RecordStore;
    runId: string;
    generation: number;
    notify: () => void;
  }) {
    this.#store = options.store;
    this.#runId = options.runId;
    this.#generation = options.generation;
    this.#notify = options.notify;
  }

  public observe(frame: Frame): void {
    if (!this.#committed) {
      this.#pending.push(frame);
      return;
    }
    this.#store.append(this.#runId, 'observation', json({ generation: this.#generation, frame }));
    this.#notify();
  }

  public commit(): void {
    this.#committed = true;
    for (const frame of this.#pending) {
      this.observe(frame);
    }
    this.#pending.length = 0;
  }
}
