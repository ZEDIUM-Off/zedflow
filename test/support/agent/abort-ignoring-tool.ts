import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool } from '@zf/core';
import { customDeclaration, resultFor } from './messages.ts';

export class AbortIgnoringTool implements ExecutableTool {
  public readonly declaration = customDeclaration;
  readonly #cwd: string;
  readonly #reject: boolean;
  readonly #released: Promise<void>;
  #release: (() => void) | undefined;
  #entered = false;
  #completed = false;
  #signal: AbortSignal | undefined;

  public constructor(cwd: string, reject: boolean) {
    this.#cwd = cwd;
    this.#reject = reject;
    this.#released = new Promise<void>((resolveReleased) => {
      this.#release = resolveReleased;
    });
  }

  public get entered(): boolean {
    return this.#entered;
  }
  public get completed(): boolean {
    return this.#completed;
  }
  public get signal(): AbortSignal | undefined {
    return this.#signal;
  }
  public release(): void {
    this.#release?.();
  }

  public async execute({
    call: input,
    signal: inputSignal,
    onUpdate,
  }: Parameters<ExecutableTool['execute']>[0]): Promise<ToolResultMessage> {
    this.#signal = inputSignal;
    onUpdate?.(resultFor(input, 'before abort'));
    this.#entered = true;
    await this.#released;
    await writeFile(join(this.#cwd, 'late-effect'), 'effects cannot be rolled back');
    onUpdate?.(resultFor(input, 'after abort'));
    this.#completed = true;
    if (this.#reject) {
      throw new Error('late fixture failure');
    }
    return resultFor(input, 'late success');
  }
}
