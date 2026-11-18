import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool, ToolExecution } from '@zf/core';
import { customTool } from './custom-tool.ts';

/** Owns an abort-ignoring call from admission through its explicitly released late effect. */
export class LateTool implements ExecutableTool {
  readonly #cwd: string;
  readonly #rejectLate: boolean;
  #announce: (() => void) | undefined;
  readonly #ready = new Promise<void>((resolveReady) => {
    this.#announce = resolveReady;
  });
  #finish: (() => void) | undefined;
  readonly #release = new Promise<void>((resolveRelease) => {
    this.#finish = resolveRelease;
  });
  #settle: (() => void) | undefined;
  readonly #settled = new Promise<void>((resolveSettled) => {
    this.#settle = resolveSettled;
  });
  readonly #late: ToolResultMessage = {
    role: 'toolResult',
    toolCallId: 'late',
    toolName: 'custom',
    timestamp: 1,
    isError: false,
    content: [{ type: 'text', text: 'late payload'.repeat(100000) }],
  };
  public readonly declaration = customTool(async () => this.#late).declaration;

  public constructor(cwd: string, rejectLate: boolean) {
    this.#cwd = cwd;
    this.#rejectLate = rejectLate;
  }

  public waitUntilStarted(): Promise<void> {
    return this.#ready;
  }

  public release(): void {
    this.#finish?.();
  }

  public waitUntilSettled(): Promise<void> {
    return this.#settled;
  }

  public async execute(input: ToolExecution): Promise<ToolResultMessage> {
    this.#announce?.();
    input.signal.addEventListener(
      'abort',
      () => {
        input.onUpdate?.(this.#late);
      },
      { once: true },
    );
    await this.#release;
    await writeFile(join(this.#cwd, 'late-effect'), 'effect after finalization');
    for (let i = 0; i < 1000; i++) {
      input.onUpdate?.(this.#late);
    }
    this.#settle?.();
    if (this.#rejectLate) {
      throw new Error('late failure after effect');
    }
    return this.#late;
  }
}
