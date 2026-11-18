import type {
  Api,
  AssistantMessage,
  JsonObject,
  Model,
  ToolCall,
  ToolResultMessage,
  TranscriptContext,
} from '@earendil-works/pi-ai';
export const png =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AP8AAP8FAAH/+lyI0QAAAABJRU5ErkJggg==';
export const message = (
  model: Model<Api>,
  content: AssistantMessage['content'],
  stopReason: AssistantMessage['stopReason'] = 'stop',
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
    output: 2,
    cacheRead: 0,
    cacheWrite: 0,
    totalTokens: 3,
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
  },
});
export const call = (name: string, args: JsonObject, id = 'call-1'): ToolCall => ({
  type: 'toolCall',
  name,
  arguments: args,
  id,
});
export const results = (request: TranscriptContext): ToolResultMessage[] =>
  request.messages.filter((item): item is ToolResultMessage => item.role === 'toolResult');
export const text = (result: ToolResultMessage | undefined): string =>
  result?.content
    .filter((item) => item.type === 'text')
    .map((item) => item.text)
    .join('\n') ?? '';
export const resultFor = (toolCall: ToolCall, value = 'ok'): ToolResultMessage => ({
  role: 'toolResult',
  toolCallId: toolCall.id,
  toolName: toolCall.name,
  content: [{ type: 'text', text: value }],
  isError: false,
  timestamp: Date.now(),
});
export const customDeclaration = {
  name: 'custom',
  description: 'Fixture custom tool',
  parameters: {
    type: 'object',
    properties: { value: { type: 'string' } },
    required: ['value'],
    additionalProperties: false,
  },
};
