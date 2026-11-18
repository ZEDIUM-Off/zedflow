import { errorText } from '../error-text.ts';
import { Worker } from 'node:worker_threads';
import type { GrepOperations } from '@earendil-works/pi-coding-agent';
import type {
  DirectoryReply,
  SearchCall,
  SearchJob,
  SearchResult,
  WorkerMessage,
} from './protocol.ts';

/** Owns one worker through actual exit, including callback and update publication windows. */
export class SearchWorkerSession {
  readonly #worker: Worker;
  readonly #operations: GrepOperations | undefined;
  readonly #call: SearchCall;
  #closed = false;
  #result: SearchResult | undefined;
  #error: Error | undefined;

  public constructor(job: SearchJob, call: SearchCall, operations: GrepOperations | undefined) {
    this.#call = call;
    this.#operations = operations;
    // A fixed module-string entry accepts inherited input-only flags without scrubbing options.
    const entry = new URL('./worker.ts', import.meta.url);
    const source = `import ${JSON.stringify(entry.href)};`;
    this.#worker = new Worker(new URL(`data:text/javascript,${encodeURIComponent(source)}`), {
      env: { ...process.env, PI_OFFLINE: '1' },
      workerData: job,
      transferList: job.call.signal === undefined ? [] : [job.call.signal],
    });
  }

  public wait(): Promise<SearchResult> {
    return new Promise((resolve, reject) => {
      this.#worker.on('message', (message: WorkerMessage) => this.#message(message));
      this.#worker.on('error', (error) => {
        this.#error = error;
      });
      this.#worker.once('exit', (code) => {
        this.#closed = true;
        if (this.#error) {
          reject(this.#error);
        } else if (code !== 0 || this.#result === undefined) {
          reject(new Error(`Search worker exited without a result (code ${code})`));
        } else {
          resolve(this.#result);
        }
      });
    });
  }

  #message(message: WorkerMessage): void {
    if (this.#closed) {
      return;
    }
    if (message.type === 'directory') {
      void this.#directory(message);
    } else if (message.type === 'update') {
      this.#publish(message.result);
    } else if (message.type === 'directory-aborted') {
      const reason: unknown = this.#call.signal?.reason;
      this.#error = new Error(errorText(reason));
    } else if (message.type === 'error') {
      this.#error = new Error(message.error);
    } else {
      this.#result = message.result;
    }
  }

  #publish(result: SearchResult): void {
    if (!this.#call.signal?.aborted && this.#result === undefined && !this.#error) {
      this.#call.onUpdate?.(result);
    }
  }

  async #directory(message: Extract<WorkerMessage, { type: 'directory' }>): Promise<void> {
    try {
      const value = await this.#operations?.isDirectory(message.path);
      this.#reply({
        id: message.id,
        value: value ?? false,
        aborted: this.#call.signal?.aborted ?? false,
      });
    } catch (error) {
      this.#reply({
        id: message.id,
        error: errorText(error),
      });
    }
  }

  #reply(reply: DirectoryReply): void {
    if (!this.#closed) {
      this.#worker.postMessage(reply);
    }
  }
}
