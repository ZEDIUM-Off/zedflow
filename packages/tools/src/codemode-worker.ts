// Adapted from Pi 1.5.2 coding-agent/src/extensions/codemode/execute.ts (MIT).
// Copyright Mario Zechner and Earendil Works; see ../LICENSE and ../PROVENANCE.json.
import { CodemodeSandbox } from '@earendil-works/pi-codemode';
import type { CodemodeTool } from '@earendil-works/pi-codemode';
import type { CodemodeAdmission } from './codemode-admission.ts';
import type { CodemodeLimits } from './codemode-limits.ts';

/** Owns a reservation from admission through worker exit, including late-start rejection. */
export class CodemodeWorker {
  readonly #signal: AbortSignal;
  readonly #admission: CodemodeAdmission;
  #sandbox: CodemodeSandbox | undefined;
  #workerExited: Promise<void> | undefined;
  #admitted = true;
  #handedToWorker = false;
  #settled = false;

  public constructor(signal: AbortSignal, admission: CodemodeAdmission) {
    this.#signal = signal;
    this.#admission = admission;
    admission.reserve();
  }

  public open(tools: CodemodeTool[], limits: CodemodeLimits, timeoutMs?: number): CodemodeSandbox {
    this.#sandbox = new CodemodeSandbox({
      tools,
      acquireWorker: () => this.#acquire(),
      onWorkerStarted: (exited) => {
        this.#workerExited = exited;
      },
      timeoutMs: limits.timeoutMs(timeoutMs),
      memoryLimitBytes: limits.memoryLimitBytes,
    });
    return this.#sandbox;
  }

  public settle(): void {
    this.#settled = true;
  }

  #acquire(): () => void {
    if (this.#settled || this.#signal.aborted || !this.#admitted) {
      throw new Error('The script already settled');
    }
    this.#handedToWorker = true;
    return () => this.#release();
  }

  #release(): void {
    if (this.#admitted) {
      this.#admitted = false;
      this.#admission.release();
    }
  }

  public async close(): Promise<void> {
    this.settle();
    await this.#sandbox?.close();
    if (this.#workerExited) {
      await this.#waitForExit(this.#workerExited);
    }
    if (this.#admitted && !this.#handedToWorker) {
      this.#release();
    }
  }

  #waitForExit(exited: Promise<void>): Promise<void> {
    // Pi unrefs terminated workers. Bound the referenced wait, not the admission lease.
    return new Promise<void>((resolveExit) => {
      const deadline = setTimeout(resolveExit, 1000);
      void exited.then(() => {
        clearTimeout(deadline);
        resolveExit();
      });
    });
  }
}
