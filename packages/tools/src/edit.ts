import { createEditToolDefinition } from '@earendil-works/pi-coding-agent';
import type { EditToolOptions } from '@earendil-works/pi-coding-agent';
import type { ExecutableTool } from '@zf/core';
import { fromPi } from './pi-tool.ts';

export const createEditTool = (cwd: string, options?: EditToolOptions): ExecutableTool =>
  fromPi(createEditToolDefinition(cwd, options));
