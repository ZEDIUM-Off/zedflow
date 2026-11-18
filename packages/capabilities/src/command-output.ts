import type { ToolResultMessage } from '@earendil-works/pi-ai';
import type { ToolExecution } from '@zf/core';

const retainedBytes = 49152;

/**
 * The output of one command tool call: the last 48 KiB of stdout and stderr together with the
 * total byte count, reported to the call observer after each chunk.
 */
export class CommandOutput {
  readonly #execution: ToolExecution;
  #tail = Buffer.alloc(0);
  #bytes = 0;

  public constructor(execution: ToolExecution) {
    this.#execution = execution;
  }

  public append(chunk: Buffer): void {
    this.#bytes += chunk.length;
    this.#tail = Buffer.concat([this.#tail, chunk]).subarray(-retainedBytes);
    try {
      this.#execution.onUpdate?.(this.#message(null, null, false));
    } catch {
      /* Observers do not own command execution. */
    }
  }

  /** The complete result: an error when the command exited with a signal or a nonzero code. */
  public result(code: number | null, signal: NodeJS.Signals | null): ToolResultMessage {
    return this.#message(code, signal, true);
  }

  #message(
    code: number | null,
    signal: NodeJS.Signals | null,
    complete: boolean,
  ): ToolResultMessage {
    const truncated = this.#bytes > this.#tail.length;
    return {
      role: 'toolResult',
      toolCallId: this.#execution.call.id,
      toolName: this.#execution.call.name,
      timestamp: Date.now(),
      isError: complete && (signal !== null || code !== 0),
      content: [
        {
          type: 'text',
          text: `${truncated ? '[Output truncated to last 48 KiB]\n' : ''}${this.#tail.toString('utf8')}`,
        },
      ],
      details: {
        exitCode: code,
        signal,
        complete,
        totalBytes: this.#bytes,
        truncated,
      },
    };
  }
}
