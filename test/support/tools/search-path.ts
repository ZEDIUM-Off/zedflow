import { join } from 'node:path';
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import { expect, vi, type Mock } from 'vite-plus/test';
import { createGrepTool } from '../../../packages/tools/src/grep.ts';
import { createFindTool } from '../../../packages/tools/src/find.ts';
import { invoke } from './invocation.ts';

interface SearchResults {
  readonly grep: ToolResultMessage;
  readonly find: ToolResultMessage;
  readonly download: Mock<() => Promise<never>>;
}

export const missingExecutable = (name: string): string =>
  `${name === 'rg' ? 'ripgrep (rg)' : name} is not available and could not be downloaded`;

export const searchWithPath = async (cwd: string, path: string): Promise<SearchResults> => {
  const download = vi.fn(async () => {
    throw new Error('Unexpected download');
  });
  vi.stubGlobal('fetch', download);
  vi.stubEnv('PATH', path);
  vi.stubEnv('PI_CODING_AGENT_DIR', join(cwd, 'empty-agent'));
  try {
    const grep = await invoke(createGrepTool(cwd), { pattern: 'anything' });
    const find = await invoke(createFindTool(cwd), { pattern: '*' });
    expect(process.env['PATH']).toBe(path);
    return { grep, find, download };
  } finally {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  }
};
