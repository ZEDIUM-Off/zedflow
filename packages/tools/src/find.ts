import { createFindToolDefinition } from '@earendil-works/pi-coding-agent';
import type { FindToolOptions } from '@earendil-works/pi-coding-agent';
import type { ExecutableTool } from '@zf/core';
import { fromPi } from './pi-tool.ts';
import { CustomFindExecution } from './search/custom-find-execution.ts';
import { SearchExecution } from './search/execution.ts';

export const createFindTool = (cwd: string, options?: FindToolOptions): ExecutableTool => {
  const operations = options?.operations ?? undefined;
  const definition = createFindToolDefinition(
    cwd,
    operations === undefined ? undefined : { operations },
  );
  return fromPi({
    ...definition,
    execute: (toolCallId, args, signal, onUpdate) =>
      (operations === undefined
        ? new SearchExecution('find', cwd)
        : new CustomFindExecution(cwd, operations)
      ).execute({ toolCallId, args, signal, onUpdate }),
  });
};
