import { createLsToolDefinition } from '@earendil-works/pi-coding-agent';
import type { LsToolOptions } from '@earendil-works/pi-coding-agent';
import type { ExecutableTool } from '@zf/core';
import { fromPi } from './pi-tool.ts';

export const createLsTool = (cwd: string, options?: LsToolOptions): ExecutableTool =>
  fromPi(createLsToolDefinition(cwd, options));
