import { createPowerShellToolDefinition } from '@earendil-works/pi-coding-agent';
import type { PowerShellToolOptions } from '@earendil-works/pi-coding-agent';
import type { ExecutableTool } from '@zf/core';
import { fromPi } from './pi-tool.ts';

export const createPowerShellTool = (
  cwd: string,
  options?: PowerShellToolOptions,
): ExecutableTool => fromPi(createPowerShellToolDefinition(cwd, options));
