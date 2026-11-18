// A single registered wait owns its abort subscription and always unregisters before settling.
export class CheckpointWait {
  readonly #waiters: Set<() => void>;
  readonly #promise: Promise<void>;
  readonly #empty: () => void;
  readonly #signal: AbortSignal | undefined;

  public constructor(
    waiters: Set<() => void>,
    ready: () => boolean,
    empty: () => void,
    signal?: AbortSignal,
  ) {
    this.#waiters = waiters;
    this.#empty = empty;
    this.#signal = signal;
    this.#promise = this.#register(ready);
  }

  public wait(): Promise<void> {
    return this.#promise;
  }

  #register(ready: () => boolean): Promise<void> {
    return new Promise((resolveWait, rejectWait) => {
      const check = (): void => {
        try {
          this.#signal?.throwIfAborted();
          if (!ready()) {
            return;
          }
          this.#cleanup(check);
          resolveWait();
        } catch (error) {
          this.#cleanup(check);
          rejectWait(error);
        }
      };
      this.#waiters.add(check);
      this.#signal?.addEventListener('abort', check, { once: true });
      check();
    });
  }

  #cleanup(check: () => void): void {
    this.#waiters.delete(check);
    this.#signal?.removeEventListener('abort', check);
    if (this.#waiters.size === 0) {
      this.#empty();
    }
  }
}
