import type { Json, RunService, RunUpdate } from '@zf/core';
import { object } from './command-json.ts';

/** Owns a turn subscription, including synchronous initial delivery and abort cleanup. */
export class HeadlessTurn {
  readonly #runs: RunService;
  readonly #id: string;
  readonly #signal: AbortSignal;
  readonly #abortListener = this.#abort.bind(this);
  #close: (() => void) | undefined;
  #settled = false;
  #resolve: ((view: Json) => void) | undefined;
  #reject: ((error: Error) => void) | undefined;
  public constructor(runs: RunService, id: string, signal: AbortSignal) {
    this.#runs = runs;
    this.#id = id;
    this.#signal = signal;
  }
  public wait(): Promise<Json> {
    return new Promise((resolve, reject) => {
      this.#resolve = resolve;
      this.#reject = reject;
      this.#signal.addEventListener('abort', this.#abortListener, { once: true });
      this.#close = this.#runs.subscribe(this.#id, (update) => this.#update(update));
      if (this.#settled) {
        this.#close();
      } else if (this.#signal.aborted) {
        this.#abort();
      }
    });
  }
  #update(update: RunUpdate): void {
    if (
      object(update.view) &&
      (update.view['status'] !== 'active' ||
        ['ready', 'failed', 'cancelled'].some(
          (state) => update.view && object(update.view) && update.view['value'] === state,
        ))
    ) {
      this.#finish();
      this.#resolve?.(update.view);
    }
  }
  #abort(): void {
    this.#finish();
    this.#reject?.(new Error('Run interrupted'));
  }
  #finish(): void {
    this.#settled = true;
    this.#close?.();
    this.#signal.removeEventListener('abort', this.#abortListener);
  }
}
