import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { expect, test } from 'vite-plus/test';
import { createCodemodeTool } from '../../packages/tools/src/codemode.ts';
import { createBuiltinTools } from '../../packages/tools/src/index.ts';
import { fixture, execute, text } from '../support/codemode/invocation.ts';
import { customTool } from '../support/codemode/custom-tool.ts';

test('Code Mode aborts actual nested execution and retains an unfinished call record', async () => {
  const controller = new AbortController();
  let observedAbort = false;
  const tool = createCodemodeTool({
    tools: [
      customTool(async (_args, signal) => {
        controller.abort(new Error('cancel test'));
        await new Promise<void>((resolveAbort) => {
          if (signal.aborted) {
            observedAbort = true;
            resolveAbort();
          } else {
            signal.addEventListener(
              'abort',
              () => {
                observedAbort = true;
                resolveAbort();
              },
              { once: true },
            );
          }
        });
        throw new Error('cancelled nested tool');
      }),
    ],
  });
  const result = await execute(tool, 'await tools.custom({value:"wait"});', controller.signal);
  expect(result.isError).toBe(true);
  expect(text(result)).toContain('aborted');
  expect(result.nestedCalls).toMatchObject({ calls: [{ name: 'custom', status: 'unfinished' }] });
  expect(observedAbort).toBe(true);
});

test('Code Mode cancels unawaited nested calls when script returns', async () => {
  let sawAbort: (() => void) | undefined;
  const aborted = new Promise<void>((resolveAbort) => {
    sawAbort = resolveAbort;
  });
  const tool = createCodemodeTool({
    tools: [
      customTool(async (_args, signal) => {
        await new Promise<void>((resolveAbort) => {
          const onAbort = () => {
            sawAbort?.();
            resolveAbort();
          };
          if (signal.aborted) {
            onAbort();
          } else {
            signal.addEventListener('abort', onAbort, { once: true });
          }
        });
        throw new Error('unawaited call aborted');
      }),
    ],
  });
  const result = await execute(tool, 'tools.custom({value:"wait"}); return "done";');
  await aborted;
  expect(result.isError).toBe(false);
  expect(text(result)).toContain('done');
  expect(result.nestedCalls).toMatchObject({ calls: [{ status: 'unfinished' }] });
});

test('Code Mode cancellation reaches a real nested bash process and retains completed tool effects', async () => {
  const cwd = await fixture();
  const controller = new AbortController();
  const tool = createCodemodeTool({ tools: createBuiltinTools({ cwd }) });
  const running = execute(
    tool,
    'await tools.write({path:"kept",content:"kept"}); await tools.bash({command:"printf started > started; sleep 2; printf BAD > forbidden"});',
    controller.signal,
  );
  let started = false;
  for (let i = 0; i < 100 && !started; i++) {
    started = await readFile(join(cwd, 'started'), 'utf8').then(
      () => true,
      () => false,
    );
    if (!started) {
      await delay(10);
    }
  }
  expect(started).toBe(true);
  controller.abort();
  const result = await running;
  expect(result.isError).toBe(true);
  expect(result.nestedCalls).toMatchObject({
    calls: [
      { name: 'write', status: 'ok' },
      { name: 'bash', status: 'unfinished' },
    ],
  });
  expect(await readFile(join(cwd, 'kept'), 'utf8')).toBe('kept');
  await delay(2100);
  await expect(readFile(join(cwd, 'forbidden'))).rejects.toThrow();
});
