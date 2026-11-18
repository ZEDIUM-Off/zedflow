import type { AssistantMessage, AssistantMessageEvent } from '@earendil-works/pi-ai';

export const assistant = (
  content: AssistantMessage['content'],
  stopReason: AssistantMessage['stopReason'] = 'stop',
): AssistantMessage => ({
  role: 'assistant',
  content,
  api: 'openai-responses',
  provider: 'openai',
  model: 'fixture',
  timestamp: 0,
  stopReason,
  usage: {
    input: 0,
    output: 0,
    cacheRead: 0,
    cacheWrite: 0,
    totalTokens: 0,
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
  },
});

export const textEvent = (text: string): AssistantMessageEvent => ({
  type: 'text_delta',
  contentIndex: 0,
  delta: text,
  partial: assistant([{ type: 'text', text }]),
});
