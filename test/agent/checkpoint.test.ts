import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { getCurrentTools, withoutInitialSystemMessage } from '@earendil-works/pi-ai';
import { expect, test } from 'vite-plus/test';
import { createMachine } from '../../packages/machines/agent/machine.ts';
import { createReadTool, createWriteTool } from '../../packages/tools/src/index.ts';
import { fixture, startMachine, context, ready } from '../support/agent/execution.ts';
import { call } from '../support/agent/messages.ts';
import { NativeOpenAI } from '../support/agent/native-openai.ts';
import { answer, calls } from '../support/agent/responses.ts';

test('agent restores a compatible native ready checkpoint with history and newly bound services, without replaying requests or tools', async () => {
  const cwd = await fixture();
  const old = await NativeOpenAI.create([
    calls(call('write', { path: 'old-effect', content: 'once' })),
    answer('old answer'),
  ]);
  const original = startMachine(
    createMachine(old.bindings(cwd, { tools: [createWriteTool(cwd)] })),
  ).execution;
  original.send({ type: 'PROMPT', text: 'old prompt' });
  await ready(original, 2);
  const history = structuredClone(context(original).messages);
  const checkpoint = original.checkpoint();
  original.stop();
  const next = await NativeOpenAI.create([answer('new answer')]);
  const replacement = startMachine(
    createMachine(
      next.bindings(cwd, {
        tools: [createReadTool(cwd)],
        config: { systemPrompt: 'New generation instructions' },
      }),
    ),
    checkpoint,
  ).execution;
  expect(context(replacement).messages).toEqual(history);
  expect(next.requestCount()).toBe(0);
  replacement.send({ type: 'PROMPT', text: 'new prompt' });
  await ready(replacement, 1);
  expect(context(replacement).messages.slice(0, history.length)).toEqual(history);
  expect(withoutInitialSystemMessage(next.request(0).messages).slice(0, history.length)).toEqual(
    history,
  );
  expect(getCurrentTools(next.request(0).messages).map((tool) => tool.name)).toEqual(['read']);
  expect(old.requestCount()).toBe(2);
  expect(await readFile(join(cwd, 'old-effect'), 'utf8')).toBe('once');
});
