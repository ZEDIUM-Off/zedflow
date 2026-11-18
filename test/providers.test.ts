import { InMemoryCredentialStore } from '@earendil-works/pi-ai';
import { StreamingProvider } from './support/providers/streaming-provider.ts';
import { openaiProvider } from '@earendil-works/pi-ai/providers/openai';
import { OpenAIRuntime } from '@zf/capabilities';
import { expect, test, vi } from 'vite-plus/test';

const authContext = { env: async () => undefined, fileExists: async () => false };
const selected = (runtime: OpenAIRuntime) => {
  const model = runtime.models.getModels('openai')[0];
  if (!model) {
    throw new Error('Pinned native OpenAI catalog is empty');
  }
  return runtime.select(model.provider, model.id);
};

test('runtime exposes native models and restricts product providers without performing network calls', async () => {
  const network = vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('Network forbidden'));
  try {
    const runtime = new OpenAIRuntime({ credentials: new InMemoryCredentialStore(), authContext });
    expect(runtime.models.getProviders().map((provider) => provider.id)).toEqual(['openai']);
    expect(selected(runtime).provider).toBe('openai');
    expect(() => runtime.select('claude', 'fixture')).toThrow('Only the OpenAI');
    expect(() => runtime.select('openai', 'nonexistent-fixture')).toThrow('zf models');
    expect(
      () =>
        new OpenAIRuntime({
          credentials: new InMemoryCredentialStore(),
          provider: { ...openaiProvider(), id: 'claude' },
        }),
    ).toThrow('Only the OpenAI');
    await expect(runtime.requireAuth(selected(runtime))).rejects.toThrow('zf auth login');
    expect(network).not.toHaveBeenCalled();
  } finally {
    network.mockRestore();
  }
});

test('native API-key login, status and logout work without mirroring provider interfaces', async () => {
  const credentials = new InMemoryCredentialStore();
  const runtime = new OpenAIRuntime({ credentials, authContext });
  const prompts: string[] = [];
  await runtime.models.login('openai', 'api_key', {
    prompt: async (prompt) => {
      prompts.push(prompt.type);
      return 'synthetic-key';
    },
    notify: () => {},
  });
  expect(prompts).toEqual(['secret']);
  await runtime.requireAuth(selected(runtime));
  expect(await runtime.models.checkAuth('openai')).toMatchObject({ type: 'api_key' });
  expect(JSON.stringify(await credentials.list())).not.toContain('synthetic-key');
  await runtime.models.logout('openai');
  await expect(runtime.requireAuth(selected(runtime))).rejects.toThrow('authentication is missing');
});

test('auth resolution failures are actionable without echoing secret-bearing upstream errors', async () => {
  const native = openaiProvider();
  const runtime = new OpenAIRuntime({
    credentials: new InMemoryCredentialStore(),
    authContext,
    provider: {
      ...native,
      auth: {
        apiKey: {
          name: 'fixture',
          resolve: async () => {
            throw new Error('synthetic-secret-must-not-escape');
          },
        },
      },
    },
  });
  try {
    await runtime.requireAuth(selected(runtime));
    throw new Error('Unexpected success');
  } catch (error) {
    expect(String(error)).toContain('zf auth login');
    expect(String(error)).not.toContain('synthetic-secret');
  }
  await expect(
    runtime.requireAuth(selected(runtime), { signal: AbortSignal.abort() }),
  ).rejects.toThrow('cancelled');
});

test('streaming and cancellation pass through native Models to the configured provider', async () => {
  const credentials = new InMemoryCredentialStore();
  await credentials.modify('openai', async () => ({ type: 'api_key', key: 'synthetic-key' }));
  const provider = new StreamingProvider();
  const runtime = new OpenAIRuntime({
    credentials,
    authContext,
    provider: provider.provider,
  });
  const events: string[] = [];
  const stream = runtime.models.streamSimple(selected(runtime), { messages: [] });
  for await (const event of stream) {
    events.push(event.type);
  }
  expect(events).toEqual(['text_delta', 'done']);
  expect(provider.receivedKey).toBe('synthetic-key');
  expect(JSON.stringify(await stream.result())).not.toContain('synthetic-key');
  const controller = new AbortController();
  const pending = runtime.models.streamSimple(
    selected(runtime),
    { messages: [{ role: 'user', content: 'wait', timestamp: 0 }] },
    { signal: controller.signal },
  );
  await provider.entered;
  controller.abort();
  expect((await pending.result()).stopReason).toBe('aborted');
});
