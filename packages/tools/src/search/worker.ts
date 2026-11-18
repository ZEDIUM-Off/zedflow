import { errorText } from '../error-text.ts';
import { parentPort, workerData } from 'node:worker_threads';
import type { MessagePort } from 'node:worker_threads';
import { DirectoryBridge } from './directory-bridge.ts';
import type { SearchJob, SearchResult, WorkerMessage } from './protocol.ts';

/** Keeps native handles alive until Pi closes them, even when its promise rejects on abort. */
class NativeSearch {
  readonly #job: SearchJob;
  readonly #port: MessagePort;
  readonly #signal: AbortSignal;
  readonly #bridge: DirectoryBridge;

  public constructor(job: SearchJob, port: MessagePort) {
    this.#job = job;
    this.#port = port;
    this.#signal = job.call.signal ?? new AbortController().signal;
    this.#bridge = new DirectoryBridge(port, this.#signal);
  }

  public async run(): Promise<void> {
    try {
      this.#signal.throwIfAborted();
      const result = await this.#execute();
      this.#send({ type: 'result', result });
    } catch (error) {
      this.#send(
        this.#bridge.abortedBeforeSpawn
          ? { type: 'directory-aborted' }
          : { type: 'error', error: errorText(error) },
      );
    } finally {
      this.#bridge.close();
      this.#port.close();
    }
  }

  async #execute(): Promise<SearchResult> {
    const pi = await import('@earendil-works/pi-coding-agent');
    this.#signal.throwIfAborted();
    const tool =
      this.#job.name === 'grep'
        ? pi.createGrepTool(this.#job.cwd, {
            operations: {
              isDirectory: (path) => this.#bridge.isDirectory(path, this.#job.remote),
            },
          })
        : pi.createFindTool(this.#job.cwd);
    return tool.execute(this.#job.call.toolCallId, this.#job.call.args, this.#signal, (result) =>
      this.#send({ type: 'update', result }),
    );
  }

  #send(message: WorkerMessage): void {
    this.#port.postMessage(message);
  }
}

if (!parentPort) {
  throw new Error('Search worker requires a parent port');
}
void new NativeSearch(workerData as SearchJob, parentPort).run();
