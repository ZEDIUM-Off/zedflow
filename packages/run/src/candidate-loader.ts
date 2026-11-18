import type { Loader, MachineSource } from '@zf/core';

export class CandidateLoader {
  readonly #loader: Loader;

  public constructor(loader: Loader) {
    this.#loader = loader;
  }

  public load(path: string, signal?: AbortSignal): Promise<MachineSource> {
    if (!signal) {
      return this.#loader.load(path);
    }
    return this.#loadGuarded(path, signal);
  }

  #loadGuarded(path: string, signal: AbortSignal): Promise<MachineSource> {
    return new Promise((resolveLoad, rejectLoad) => {
      const abort = (): void => {
        signal.removeEventListener('abort', abort);
        rejectLoad(signal.reason);
      };
      if (signal.aborted) {
        abort();
        return;
      }
      signal.addEventListener('abort', abort, { once: true });
      // A cancelled preparation is never activated, even if its trusted import later settles.
      void Promise.resolve()
        .then(() => {
          signal.throwIfAborted();
          return this.#loader.load(path);
        })
        .then(
          (source) => {
            signal.removeEventListener('abort', abort);
            resolveLoad(source);
          },
          (error: unknown) => {
            signal.removeEventListener('abort', abort);
            rejectLoad(error);
          },
        );
    });
  }
}
