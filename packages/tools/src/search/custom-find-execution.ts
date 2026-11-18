import { createFindTool as createPiFindTool } from '@earendil-works/pi-coding-agent';
import type { FindOperations } from '@earendil-works/pi-coding-agent';
import { SearchExecution } from './execution.ts';
import type { SearchCall, SearchResult } from './protocol.ts';

/** Owns one native custom-find discriminator and its caller-realm operation receiver. */
export class CustomFindExecution implements FindOperations {
  readonly #cwd: string;
  readonly #operations: FindOperations;
  readonly #fallback = new Error('Local native find required');
  #observed = false;

  public constructor(cwd: string, operations: FindOperations) {
    this.#cwd = cwd;
    this.#operations = operations;
  }

  public async execute(call: SearchCall): Promise<SearchResult> {
    try {
      return await createPiFindTool(this.#cwd, { operations: this }).execute(
        call.toolCallId,
        call.args,
        call.signal,
        call.onUpdate,
      );
    } catch (error) {
      if (error !== this.#fallback) {
        throw error;
      }
      return new SearchExecution('find', this.#cwd).execute(call);
    }
  }

  public get glob(): FindOperations['glob'] {
    if (!this.#observed) {
      this.#observed = true;
      // Consume Pi's first lookup inside its abort/settlement window. Never enter ensureTool.
      if (!this.#operations.glob) {
        throw this.#fallback;
      }
    }
    return this.#glob.bind(this);
  }

  public exists(path: string): ReturnType<FindOperations['exists']> {
    const ops = this.#operations;
    return ops.exists(path);
  }

  #glob(
    pattern: string,
    cwd: string,
    options: Parameters<FindOperations['glob']>[2],
  ): ReturnType<FindOperations['glob']> {
    // Pi reacquires glob after awaiting exists; neither the getter nor method is snapshotted.
    const ops = this.#operations;
    return ops.glob(pattern, cwd, options);
  }
}
