import type { ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool } from '@zf/core';

export class ObservedTool implements ExecutableTool {
  readonly #tool: ExecutableTool;
  #settled = false;

  public constructor(tool: ExecutableTool) {
    this.#tool = tool;
  }
  public get declaration(): ExecutableTool['declaration'] {
    return this.#tool.declaration;
  }
  public get settled(): boolean {
    return this.#settled;
  }

  public async execute(
    input: Parameters<ExecutableTool['execute']>[0],
  ): Promise<ToolResultMessage> {
    try {
      return await this.#tool.execute(input);
    } finally {
      this.#settled = true;
    }
  }
}
