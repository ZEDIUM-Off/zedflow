import type { ExecutableTool } from '@zf/core';
import { createBashTool } from './bash.ts';
import { createEditTool } from './edit.ts';
import { createFindTool } from './find.ts';
import { createGrepTool } from './grep.ts';
import { createLsTool } from './ls.ts';
import { createPowerShellTool } from './powershell.ts';
import { createReadTool } from './read.ts';
import { createWriteTool } from './write.ts';

export interface BuiltinToolsOptions {
  readonly cwd: string;
}
export const createBuiltinTools = (options: BuiltinToolsOptions): readonly ExecutableTool[] => [
  createReadTool(options.cwd),
  createWriteTool(options.cwd),
  createEditTool(options.cwd),
  createBashTool(options.cwd),
  createGrepTool(options.cwd),
  createFindTool(options.cwd),
  createLsTool(options.cwd),
  ...(process.platform === 'win32' ? [createPowerShellTool(options.cwd)] : []),
];
