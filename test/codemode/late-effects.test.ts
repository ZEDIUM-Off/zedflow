import { LateTool } from '../support/codemode/late-tool.ts';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import { expect, test } from 'vite-plus/test';
import { createCodemodeTool } from '../../packages/tools/src/codemode.ts';
import { fixture, execute, text } from '../support/codemode/invocation.ts';

test.each([
  { ending: 'abort', rejectLate: false },
  { ending: 'abort', rejectLate: true },
  { ending: 'return', rejectLate: false },
  { ending: 'return', rejectLate: true },
])(
  'Code Mode suppresses abort-ignoring late tool updates/results and reports uncertain effects: $ending, rejection=$rejectLate',
  async ({ ending, rejectLate }) => {
    const cwd = await fixture();
    const controller = new AbortController();
    const updates: ToolResultMessage[] = [];
    const nested = new LateTool(cwd, rejectLate);
    const tool = createCodemodeTool({ tools: [nested] });
    const running = execute(
      tool,
      ending === 'abort'
        ? 'await tools.custom({value:"wait"});'
        : 'tools.custom({value:"wait"}); return "script ended";',
      controller.signal,
      (update) => updates.push(update),
    );
    await nested.waitUntilStarted();
    if (ending === 'abort') {
      controller.abort();
    }
    const result = await running;
    const updateCount = updates.length;
    const snapshot = JSON.stringify(result);
    expect(result.isError).toBe(ending === 'abort');
    expect(result.nestedCalls).toMatchObject({
      calls: [{ name: 'custom', status: 'unfinished' }],
      complete: false,
    });
    expect(tool.declaration.description).toContain('unfinished/uncertain');
    expect(text(result)).toContain('unfinished/uncertain');
    nested.release();
    await nested.waitUntilSettled();
    await delay(30);
    expect(await readFile(join(cwd, 'late-effect'), 'utf8')).toBe('effect after finalization');
    expect(updates).toHaveLength(updateCount);
    expect(JSON.stringify(result)).toBe(snapshot);
    expect(updates.every((update) => JSON.stringify(update).length < 3000)).toBe(true);
  },
);
