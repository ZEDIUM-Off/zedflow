import { errorText } from './terminal-notices.ts';
import type { TerminalNotices } from './terminal-notices.ts';

/** Owns cancellation registration and settlement of UI-side command work. */
export class TerminalOperationScope {
  readonly #notices: TerminalNotices;
  readonly #controllers = new Set<AbortController>();
  readonly #operations = new Set<Promise<void>>();

  public constructor(notices: TerminalNotices) {
    this.#notices = notices;
  }

  public controller(): AbortController {
    const controller = new AbortController();
    this.#controllers.add(controller);
    return controller;
  }

  public release(controller: AbortController): void {
    this.#controllers.delete(controller);
  }

  public track(operation: Promise<void>): void {
    const tracked = operation
      .catch((error: unknown) => this.#notices.add(`Command failed: ${errorText(error)}`))
      .finally(() => this.#operations.delete(tracked));
    this.#operations.add(tracked);
  }

  public abort(): void {
    for (const controller of this.#controllers) {
      controller.abort();
    }
  }

  public async settle(): Promise<void> {
    await Promise.allSettled(this.#operations);
  }
}
