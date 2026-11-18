import type { ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool } from '@zf/core';
import { customDeclaration, resultFor } from './messages.ts';

export class AbortResolvingTool implements ExecutableTool {
  public readonly declaration = customDeclaration;
  #entered = false;

  public get entered(): boolean {
    return this.#entered;
  }

  public execute({
    call: input,
    signal,
  }: Parameters<ExecutableTool['execute']>[0]): Promise<ToolResultMessage> {
    return new Promise<ToolResultMessage>((resolveResult) => {
      signal.addEventListener('abort', () => resolveResult(resultFor(input, 'late success')), {
        once: true,
      });
      this.#entered = true;
    });
  }
}
