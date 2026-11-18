import type { RunService } from '@zf/core';
import type { TerminalChrome } from './terminal-chrome.ts';
import { errorText } from './terminal-notices.ts';
import type { TerminalNotices } from './terminal-notices.ts';
import type { TerminalOperationScope } from './terminal-operation-scope.ts';

/** Admits one reload at a time and releases its cancellation registration on settlement. */
export class TerminalReload {
  readonly #runs: RunService;
  readonly #runId: string;
  readonly #notices: TerminalNotices;
  readonly #chrome: TerminalChrome;
  #pending: AbortController | undefined;

  public constructor(
    runs: RunService,
    runId: string,
    notices: TerminalNotices,
    chrome: TerminalChrome,
  ) {
    this.#runs = runs;
    this.#runId = runId;
    this.#notices = notices;
    this.#chrome = chrome;
  }

  public async run(scope: TerminalOperationScope): Promise<void> {
    if (this.#pending) {
      this.#notices.add('A reload is already pending.');
      return;
    }
    const controller = scope.controller();
    this.#pending = controller;
    this.#chrome.reloading(true);
    this.#notices.add('Reload requested; it starts once the active work finishes.');
    try {
      await this.#runs.reload(this.#runId, { wait: true, signal: controller.signal });
      this.#notices.add('Reload applied.');
    } catch (error) {
      this.#notices.add(
        controller.signal.aborted ? 'Reload cancelled.' : `Reload refused: ${errorText(error)}`,
      );
    } finally {
      this.#release(scope, controller);
    }
  }

  #release(scope: TerminalOperationScope, controller: AbortController): void {
    scope.release(controller);
    this.#pending = undefined;
    this.#chrome.reloading(false);
  }
}
