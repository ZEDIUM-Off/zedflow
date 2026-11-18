import { createReadToolDefinition } from '@earendil-works/pi-coding-agent';
import type { ReadToolOptions as PiReadToolOptions } from '@earendil-works/pi-coding-agent';
import type { Api, Model, ModelImageResizeOptions } from '@earendil-works/pi-ai';
import type { ExecutableTool } from '@zf/core';
import { fromPi } from './pi-tool.ts';

export interface ReadToolOptions extends Omit<
  PiReadToolOptions,
  'autoResizeImages' | 'resizeOptions'
> {
  readonly model?: Model<Api>;
  autoResizeImages?: boolean | undefined;
  resizeOptions?: ModelImageResizeOptions | undefined;
}

/** Observes the caller's model even after native read awaits file operations. */
class ReadModelContext {
  readonly #options: ReadToolOptions | undefined;

  public constructor(options: ReadToolOptions | undefined) {
    this.#options = options;
  }

  public get model(): Model<Api> | undefined {
    return this.#options?.model;
  }
}

export const createReadTool = (cwd: string, options?: ReadToolOptions): ExecutableTool => {
  // Historical callers may explicitly pass undefined; native optional properties omit it.
  const { autoResizeImages, resizeOptions, operations } = options ?? {};
  const nativeOptions: PiReadToolOptions = {
    ...(autoResizeImages === undefined ? {} : { autoResizeImages }),
    ...(resizeOptions === undefined ? {} : { resizeOptions }),
    ...(operations === undefined ? {} : { operations }),
  };
  return fromPi(
    createReadToolDefinition(cwd, nativeOptions),
    undefined,
    new ReadModelContext(options),
  );
};
