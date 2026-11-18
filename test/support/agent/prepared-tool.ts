import type { ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool } from '@zf/core';
import { expect } from 'vite-plus/test';
import { call, customDeclaration, png, resultFor } from './messages.ts';

export class PreparedTool implements ExecutableTool {
  #preparations = 0;
  #lateUpdate: ((result: ToolResultMessage) => void) | undefined;
  readonly #rich: ToolResultMessage = {
    ...resultFor(call('custom', {})),
    content: [
      { type: 'text', text: 'rich result' },
      { type: 'image', mimeType: 'image/png', data: png },
    ],
    details: { values: [1, 'two', false] },
    durationMs: 3,
    nestedCalls: {
      calls: [
        {
          id: 'inner-1',
          name: 'inner',
          arguments: { value: 'nested' },
          status: 'ok',
          durationMs: 1,
        },
      ],
      complete: true,
    },
  };
  public readonly declaration = {
    ...customDeclaration,
    parameters: {
      type: 'object',
      properties: { value: { type: 'string' }, count: { type: 'integer' } },
      required: ['value', 'count'],
      additionalProperties: false,
    },
  };

  public get rich(): ToolResultMessage {
    return this.#rich;
  }

  public get preparations(): number {
    return this.#preparations;
  }

  public prepareArguments(args: unknown): Record<string, unknown> {
    this.#preparations++;
    const copy = args as Record<string, unknown>;
    copy['value'] = `${String(copy['value'])}!`;
    return copy;
  }

  public async execute({
    call: preparedCall,
    argumentsPrepared,
    onUpdate,
  }: Parameters<ExecutableTool['execute']>[0]): Promise<ToolResultMessage> {
    expect(argumentsPrepared).toBe(true);
    expect(preparedCall.arguments).toEqual({ value: 'raw!', count: 2 });
    this.#lateUpdate = onUpdate;
    onUpdate?.(this.#rich);
    return this.#rich;
  }

  public emitFinalizedUpdate(): void {
    this.#lateUpdate?.(this.#rich);
  }
}
