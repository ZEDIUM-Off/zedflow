import type { Message, ToolCall } from '@earendil-works/pi-ai';

export interface AgentContext {
  readonly messages: Message[];
  readonly pending: ToolCall[];
  readonly modelCalls: number;
  readonly toolCalls: number;
  readonly failure: string | null;
}
export const initial = (): AgentContext => ({
  messages: [],
  pending: [],
  modelCalls: 0,
  toolCalls: 0,
  failure: null,
});

export const startPrompt = ({
  context,
  event,
}: {
  context: AgentContext;
  event: { text: string };
}): { target: 'thinking'; context: AgentContext } => ({
  target: 'thinking',
  context: {
    ...context,
    messages: [...context.messages, { role: 'user', content: event.text, timestamp: 0 }],
    pending: [],
    modelCalls: 0,
    toolCalls: 0,
    failure: null,
  },
});
export const cancelTurn = ({
  context,
}: {
  context: AgentContext;
}): { target: 'cancelled'; context: AgentContext } => ({
  target: 'cancelled',
  context: {
    ...context,
    pending: [],
    failure: 'Cancelled; in-flight tool effects may be incomplete',
  },
});
