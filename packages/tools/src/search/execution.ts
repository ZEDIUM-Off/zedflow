import { transferableAbortController } from 'node:util';
import type { GrepOperations } from '@earendil-works/pi-coding-agent';
import type { SearchCall, SearchResult } from './protocol.ts';
import { SearchWorkerSession } from './worker-session.ts';

/** Owns offline worker execution and signal transfer through actual native child exit. */
export class SearchExecution {
  readonly #name: 'grep' | 'find';
  readonly #cwd: string;
  readonly #operations: GrepOperations | undefined;
  readonly #controller = transferableAbortController();

  public constructor(name: 'grep' | 'find', cwd: string, operations?: GrepOperations) {
    this.#name = name;
    this.#cwd = cwd;
    this.#operations = operations;
  }

  public async execute(call: SearchCall): Promise<SearchResult> {
    // Caller-owned reasons may not be cloneable; only cancellation crosses the worker boundary.
    const abort = (): void => this.#controller.abort();
    call.signal?.addEventListener('abort', abort, { once: true });
    try {
      call.signal?.throwIfAborted();
      return await new SearchWorkerSession(
        {
          name: this.#name,
          cwd: this.#cwd,
          remote: this.#operations !== undefined,
          call: { toolCallId: call.toolCallId, args: call.args, signal: this.#controller.signal },
        },
        call,
        this.#operations,
      ).wait();
    } finally {
      call.signal?.removeEventListener('abort', abort);
    }
  }
}
