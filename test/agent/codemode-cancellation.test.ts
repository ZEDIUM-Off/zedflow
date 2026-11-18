import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import { expect, test, vi } from 'vite-plus/test';
import { createMachine } from '../../packages/machines/agent/machine.ts';
import { createCodemodeTool, createWriteTool } from '../../packages/tools/src/index.ts';
import { fixture, startMachine, context, ready } from '../support/agent/execution.ts';
import { call, results, text } from '../support/agent/messages.ts';
import { NativeOpenAI } from '../support/agent/native-openai.ts';
import { ObservedTool } from '../support/agent/observed-tool.ts';
import { answer, calls } from '../support/agent/responses.ts';

test('agent cancels an actual Code Mode worker and reuses the same tool without replaying the cancelled turn', async () => {
  const cwd = await fixture();
  const codemode = createCodemodeTool({
    tools: [createWriteTool(cwd)],
    artifactDirectory: join(cwd, 'output'),
    maxConcurrentExecutions: 1,
  });
  const observed = new ObservedTool(codemode);
  const native = await NativeOpenAI.create([
    calls(
      call(
        'codemode',
        { code: 'await tools.write({path:"vm-started", content:"started"}); while (true) {}' },
        'cancelled-script',
      ),
    ),
    calls(call('codemode', { code: 'return 42;' }, 'fresh-script')),
    answer('after VM cancellation'),
  ]);
  const updates: ToolResultMessage[] = [];
  const { execution } = startMachine(
    createMachine(
      native.bindings(cwd, { tools: [observed], onToolUpdate: (update) => updates.push(update) }),
    ),
  );
  execution.send({ type: 'PROMPT', text: 'start the VM' });
  await vi.waitFor(
    async () => expect(await readFile(join(cwd, 'vm-started'), 'utf8')).toBe('started'),
    { timeout: 10000 },
  );
  execution.send({ type: 'CANCEL' });
  const cancelled = execution.view();
  const count = updates.length;
  await vi.waitFor(() => expect(observed.settled).toBe(true), { timeout: 10000 });
  expect(execution.view()).toEqual(cancelled);
  expect(updates).toHaveLength(count);
  expect(native.requestCount()).toBe(1);
  execution.send({ type: 'RESET' });
  execution.send({ type: 'PROMPT', text: 'reuse the VM tool' });
  await ready(execution, 2);
  expect(results(native.request(2))[0]).toMatchObject({
    toolCallId: 'fresh-script',
    isError: false,
  });
  expect(text(results(native.request(2))[0])).toContain('42');
  expect(context(execution).messages.filter((item) => item.role === 'toolResult')).toHaveLength(1);
});
