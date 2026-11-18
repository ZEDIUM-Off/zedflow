import { spawn } from 'node:child_process';
import type { ChildProcessWithoutNullStreams } from 'node:child_process';
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import type { ToolDeclaration, ToolExecution } from '@zf/core';
import { CommandOutput } from './command-output.ts';

type CommandExecution = Extract<ToolDeclaration['execution'], { readonly command: string }>;

const killDelay = 1000;

/**
 * One spawned command of a tool call. It owns the process group, its bounded output and its
 * cancellation: an abort sends SIGTERM to the group, then SIGKILL after one second. The abort
 * listener and the kill timer are released when the process closes or fails to start.
 */
export class CommandRun {
  readonly #child: ChildProcessWithoutNullStreams;
  readonly #signal: AbortSignal;
  readonly #output: CommandOutput;
  /** One listener reference, so that it can be removed once the process settles. */
  readonly #abortListener: () => void;
  #killTimer: ReturnType<typeof setTimeout> | undefined;

  public constructor(execution: CommandExecution, cwd: string, input: ToolExecution) {
    this.#child = spawn(execution.command, [...execution.args], {
      cwd,
      stdio: ['pipe', 'pipe', 'pipe'],
      detached: process.platform !== 'win32',
    });
    this.#signal = input.signal;
    this.#output = new CommandOutput(input);
    this.#abortListener = (): void => {
      this.#abort();
    };
  }

  /** Writes `stdin` unless the call is already cancelled, and settles when the process closes. */
  public result(stdin: string): Promise<ToolResultMessage> {
    return new Promise((resolveResult, rejectResult) => {
      const collect = (chunk: Buffer): void => {
        this.#output.append(chunk);
      };
      this.#child.stdout.on('data', collect);
      this.#child.stderr.on('data', collect);
      this.#child.stdin.on('error', () => {
        /* A command may exit without reading its input. */
      });
      this.#child.once('error', () => {
        this.#release();
        rejectResult(new Error('Command tool failed to start'));
      });
      this.#child.once('close', (code, signal) => {
        this.#release();
        if (this.#signal.aborted) {
          rejectResult(new Error('Command tool cancelled'));
        } else {
          resolveResult(this.#output.result(code, signal));
        }
      });
      this.#signal.addEventListener('abort', this.#abortListener, { once: true });
      if (this.#signal.aborted) {
        this.#abort();
      } else {
        this.#child.stdin.end(stdin);
      }
    });
  }

  #abort(): void {
    this.#kill('SIGTERM');
    this.#killTimer = setTimeout(() => {
      this.#kill('SIGKILL');
    }, killDelay);
    this.#killTimer.unref();
  }

  #kill(signal: NodeJS.Signals): void {
    try {
      if (process.platform !== 'win32' && this.#child.pid) {
        process.kill(-this.#child.pid, signal);
      } else {
        this.#child.kill(signal);
      }
    } catch {
      /* It may already have exited. */
    }
  }

  #release(): void {
    this.#signal.removeEventListener('abort', this.#abortListener);
    if (this.#killTimer) {
      clearTimeout(this.#killTimer);
    }
  }
}
