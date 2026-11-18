import { stat } from 'node:fs/promises';
import type { MessagePort } from 'node:worker_threads';
import type { DirectoryReply } from './protocol.ts';

/** Owns realm crossings; caller settlement is awaited without starting post-abort rg. */
export class DirectoryBridge {
  readonly #port: MessagePort;
  readonly #signal: AbortSignal;
  readonly #pending = new Map<
    number,
    { resolve: (value: boolean) => void; reject: (error: Error) => void }
  >();
  #sequence = 0;
  #abortedBeforeSpawn = false;
  #closed = false;

  public constructor(port: MessagePort, signal: AbortSignal) {
    this.#port = port;
    this.#signal = signal;
    port.on('message', (reply: DirectoryReply) => this.#reply(reply));
  }

  public async isDirectory(path: string, remote: boolean): Promise<boolean> {
    const value = remote ? await this.#request(path) : (await stat(path)).isDirectory();
    // Pi grep lacks this check. Preserve the callback await, but never start rg after abort.
    if (this.#signal.aborted) {
      this.#abortedBeforeSpawn = true;
      this.#signal.throwIfAborted();
    }
    return value;
  }

  public get abortedBeforeSpawn(): boolean {
    return this.#abortedBeforeSpawn;
  }

  public close(): void {
    this.#closed = true;
    this.#pending.clear();
  }

  #request(path: string): Promise<boolean> {
    return new Promise((resolve, reject) => {
      if (!this.#closed) {
        const id = ++this.#sequence;
        this.#pending.set(id, { resolve, reject });
        this.#port.postMessage({ type: 'directory', id, path });
      }
    });
  }

  #reply(reply: DirectoryReply): void {
    const pending = this.#pending.get(reply.id);
    this.#pending.delete(reply.id);
    if (this.#closed) {
      return;
    }
    if ('error' in reply) {
      pending?.reject(new Error(reply.error));
    } else if (reply.aborted) {
      // Signal transfer and callback replies use separate queues; carry this boundary atomically.
      this.#abortedBeforeSpawn = true;
      pending?.reject(new Error('Operation aborted'));
    } else {
      pending?.resolve(reply.value);
    }
  }
}
