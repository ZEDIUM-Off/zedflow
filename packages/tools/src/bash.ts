import { createBashToolDefinition } from '@earendil-works/pi-coding-agent';
import type { BashToolOptions as PiBashToolOptions } from '@earendil-works/pi-coding-agent';
import type { ExecutableTool } from '@zf/core';
import { fromPi } from './pi-tool.ts';

export interface BashToolOptions extends Omit<PiBashToolOptions, 'commandPrefix' | 'shellPath'> {
  commandPrefix?: string | undefined;
  shellPath?: string | undefined;
}

const toPiBashOptions = (options: BashToolOptions): PiBashToolOptions => {
  const operations = options.operations;
  // Match native observation order, including skipping shellPath for custom operations.
  const shellPath = operations === undefined ? options.shellPath : undefined;
  const commandPrefix = options.commandPrefix;
  const exposeSessionEnvironment = options.exposeSessionEnvironment;
  const spawnHook = options.spawnHook;
  return {
    ...(operations === undefined ? {} : { operations }),
    ...(shellPath === undefined ? {} : { shellPath }),
    ...(commandPrefix === undefined ? {} : { commandPrefix }),
    ...(exposeSessionEnvironment === undefined ? {} : { exposeSessionEnvironment }),
    ...(spawnHook === undefined ? {} : { spawnHook }),
  };
};

export const createBashTool = (cwd: string, options?: BashToolOptions): ExecutableTool => {
  const nativeOptions =
    options === undefined || options === null ? undefined : toPiBashOptions(options);
  return fromPi(createBashToolDefinition(cwd, nativeOptions));
};
