import type { AssistantMessage, Provider } from '@earendil-works/pi-ai';

export const assistantMessage = (
  model: Parameters<Provider['streamSimple']>[0],
  content: AssistantMessage['content'],
  stopReason: AssistantMessage['stopReason'],
): AssistantMessage => ({
  role: 'assistant',
  content,
  stopReason,
  api: model.api,
  provider: model.provider,
  model: model.id,
  timestamp: Date.now(),
  usage: {
    input: 1,
    output: 1,
    cacheRead: 0,
    cacheWrite: 0,
    totalTokens: 2,
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
  },
});

export const toolCallContent = (mode: 'direct' | 'only'): AssistantMessage['content'] => [
  {
    type: 'toolCall',
    id: 'fixture-call',
    name: mode === 'only' ? 'codemode' : 'write',
    arguments:
      mode === 'only'
        ? { code: `text(await tools.write({path:'generated.txt',content:'native fixture'}));` }
        : { path: 'generated.txt', content: 'native fixture' },
  },
];
