import { createGrepToolDefinition } from '@earendil-works/pi-coding-agent';
import type { GrepToolOptions } from '@earendil-works/pi-coding-agent';
import type { ExecutableTool } from '@zf/core';
import { fromPi } from './pi-tool.ts';
import { SearchExecution } from './search/execution.ts';

export const createGrepTool = (cwd: string, options?: GrepToolOptions): ExecutableTool => {
  const operations = options?.operations ?? undefined;
  const definition = createGrepToolDefinition(
    cwd,
    operations === undefined ? undefined : { operations },
  );
  return fromPi({
    ...definition,
    execute: (toolCallId, args, signal, onUpdate) =>
      new SearchExecution('grep', cwd, operations).execute({ toolCallId, args, signal, onUpdate }),
  });
};
