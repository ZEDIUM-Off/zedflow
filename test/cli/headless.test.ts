import { createAssistantMessageEventStream } from '@earendil-works/pi-ai';
import { type AssistantMessage, type Provider } from '@earendil-works/pi-ai';
import { openaiProvider } from '@earendil-works/pi-ai/providers/openai';
import { expect, test } from 'vite-plus/test';
import { Command } from '../../packages/cli/src/index.ts';
import { fixture, isolatedAuth } from '../support/cli/fixture.ts';
import { scriptedProvider } from '../support/cli/scripted-provider.ts';
import { guardNetwork } from '../support/cli/network-guard.ts';

guardNetwork();

test('headless cancellation stops the native effect without replay or dangling signal handlers', async () => {
  const f = await fixture();
  const abort = new AbortController();
  let effectAborted = false;
  let calls = 0;
  const entry: { resolve?: () => void } = {};
  const entered = new Promise<void>((resolveEntered) => {
    entry.resolve = resolveEntered;
  });
  const provider: Provider = {
    ...openaiProvider(),
    streamSimple: (_model, _context, options) => {
      calls++;
      options?.signal?.addEventListener(
        'abort',
        () => {
          effectAborted = true;
        },
        { once: true },
      );
      entry.resolve?.();
      return createAssistantMessageEventStream();
    },
  };
  const output: string[] = [];
  const login = new Command({
    provider,
    authContext: isolatedAuth,
    interaction: { prompt: async () => 'synthetic-cancel-key', notify: () => undefined },
    write: (text) => output.push(text),
    error: (text) => output.push(text),
  });
  expect(await login.run([...f.flags, 'auth', 'login', '--api-key'])).toBe(0);
  const before = process.listenerCount('SIGINT');
  const command = new Command({
    provider,
    authContext: isolatedAuth,
    signal: abort.signal,
    write: (text) => output.push(text),
    error: (text) => output.push(text),
  });
  const pending = command.run([...f.flags, 'run', 'agent', '--prompt', 'wait']);
  await entered;
  abort.abort();
  expect(await pending).toBe(130);
  expect(calls).toBe(1);
  expect(effectAborted).toBe(true);
  expect(process.listenerCount('SIGINT')).toBe(before);
});

test('failed headless provider and budget turns emit their reasons on stderr', async () => {
  const f = await fixture();
  let error = '';
  const provider: Provider = {
    ...openaiProvider(),
    streamSimple: (model) => {
      const stream = createAssistantMessageEventStream();
      const message: AssistantMessage = {
        role: 'assistant',
        content: [],
        stopReason: 'error',
        errorMessage: 'offline provider failure',
        api: model.api,
        provider: model.provider,
        model: model.id,
        timestamp: 0,
        usage: {
          input: 0,
          output: 0,
          cacheRead: 0,
          cacheWrite: 0,
          totalTokens: 0,
          cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
        },
      };
      stream.push({ type: 'error', reason: 'error', error: message });
      return stream;
    },
  };
  const options = {
    provider,
    authContext: isolatedAuth,
    interaction: { prompt: async () => 'synthetic-error-key', notify: () => undefined },
    write: () => undefined,
    error: (text: string) => {
      error += text;
    },
  };
  const command = new Command(options);
  expect(await command.run([...f.flags, 'auth', 'login', '--api-key'])).toBe(0);
  expect(await command.run([...f.flags, 'run', 'agent', '--prompt', 'fail'])).toBe(1);
  expect(error).toContain('offline provider failure');
  error = '';
  const budget = new Command({ ...options, provider: scriptedProvider('direct').provider });
  expect(
    await budget.run([...f.flags, 'run', 'agent', '--prompt', 'budget', '--max-model-calls', '1']),
  ).toBe(1);
  expect(error).toMatch(/budget|limit/i);
});
