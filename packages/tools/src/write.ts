import { createWriteToolDefinition } from '@earendil-works/pi-coding-agent';
import type { WriteToolOptions } from '@earendil-works/pi-coding-agent';
import type { ExecutableTool } from '@zf/core';
import { fromPi } from './pi-tool.ts';

export const createWriteTool = (cwd: string, options?: WriteToolOptions): ExecutableTool =>
  fromPi(createWriteToolDefinition(cwd, options));
