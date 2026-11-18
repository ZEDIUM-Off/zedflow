/** Owns process signal handlers for exactly one invocation, including parse/help failures. */
export class CommandLifecycle {
  public readonly abort = new AbortController();
  public readonly signal: AbortSignal;
  readonly #interrupt = this.#cancel.bind(this);
  public constructor(signal?: AbortSignal) {
    process.on('SIGINT', this.#interrupt);
    process.on('SIGTERM', this.#interrupt);
    this.signal = signal ? AbortSignal.any([this.abort.signal, signal]) : this.abort.signal;
  }
  public close(): void {
    process.off('SIGINT', this.#interrupt);
    process.off('SIGTERM', this.#interrupt);
  }
  #cancel(): void {
    this.abort.abort();
  }
}
