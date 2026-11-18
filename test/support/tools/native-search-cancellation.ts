import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { createGrepTool, createFindTool } from '../../../packages/tools/src/index.ts';
import type { NativeSearchFixture } from './native-search-fixture.ts';
import { invoke, text } from './invocation.ts';

const gone = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return false;
  } catch (error) {
    if (error instanceof Error && 'code' in error && error.code === 'ESRCH') {
      return true;
    }
    throw error;
  }
};

export interface CancellationEvidence {
  readonly isError: boolean;
  readonly errorText: string;
  readonly pid: number;
  readonly startedAt: number;
  readonly abortedAt: number;
  readonly settledAt: number;
}

/** Owns cancellation observations and releases its fixture's blocking resource on failure. */
export class NativeSearchCancellation {
  readonly #box: NativeSearchFixture;
  readonly #name: 'rg' | 'fd';
  readonly #controller = new AbortController();
  readonly #updates: unknown[] = [];
  #pid: number | undefined;
  #startedAt = 0;
  #abortedAt = 0;

  public constructor(box: NativeSearchFixture, name: 'rg' | 'fd') {
    this.#box = box;
    this.#name = name;
  }

  public async run(reason?: unknown): Promise<CancellationEvidence> {
    const tool =
      this.#name === 'rg' ? createGrepTool(this.#box.cwd) : createFindTool(this.#box.cwd);
    const running = invoke(tool, { pattern: 'BLOCK_MARKER' }, this.#controller.signal, (update) =>
      this.#updates.push(update),
    );
    try {
      this.#pid = await this.#started();
      this.#abort(reason);
      const result = await running;
      assert.equal(text(result), 'Operation aborted');
      assert(result.isError);
      assert(gone(this.#pid), 'Native subprocess still alive at delivery');
      assert.deepEqual(this.#updates, []);
      return {
        isError: result.isError,
        errorText: text(result),
        pid: this.#pid,
        startedAt: this.#startedAt,
        abortedAt: this.#abortedAt,
        settledAt: Date.now(),
      };
    } finally {
      await this.#cleanup();
    }
  }

  #abort(reason: unknown): void {
    assert(this.#pid !== undefined && !gone(this.#pid), 'Native subprocess not running at abort');
    this.#abortedAt = Date.now();
    this.#controller.abort(reason);
    if (reason !== undefined) {
      assert.equal(this.#controller.signal.reason, reason);
    }
  }

  async #started(): Promise<number> {
    for (let attempt = 0; attempt < 600; attempt++) {
      const value = await readFile(join(this.#box.root, `${this.#name}.pid`), 'utf8').catch(
        () => '',
      );
      if (Number(value) > 0) {
        this.#startedAt = Date.now();
        return Number(value);
      }
      await delay(10);
    }
    throw new Error('Native process never started');
  }

  async #cleanup(): Promise<void> {
    this.#controller.abort();
    await this.#box.releaseBlockingSearch();
  }
}
