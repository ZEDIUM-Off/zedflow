import type { Message, ToolCall } from '@earendil-works/pi-ai';
import type { AgentContext, AgentEvent } from './machine.ts';

export const initialContext = (): AgentContext => ({
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
  event: Extract<AgentEvent, { type: 'PROMPT' }>;
}): { target: 'thinking'; context: AgentContext } | undefined => {
  if (typeof event.text !== 'string' || !event.text.trim()) {
    return undefined;
  }
  return {
    target: 'thinking',
    context: {
      ...context,
      messages: [...context.messages, { role: 'user', content: event.text, timestamp: Date.now() }],
      pending: [],
      modelCalls: 0,
      toolCalls: 0,
      failure: null,
    },
  };
};

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

export const countModelCall = ({
  context,
}: {
  context: AgentContext;
}): { context: AgentContext } => ({
  context: { ...context, modelCalls: context.modelCalls + 1 },
});

export const countToolCall = ({
  context,
}: {
  context: AgentContext;
}): { context: AgentContext } => ({
  context: { ...context, toolCalls: context.toolCalls + 1 },
});

export const requestMessages = ({ context }: { context: AgentContext }): Message[] =>
  structuredClone(context.messages);

export const nextCall = ({ context }: { context: AgentContext }): ToolCall => {
  const call = context.pending[0];
  if (!call) {
    throw new Error('Missing pending tool call');
  }
  return call;
};

export const nextTool = ({
  context,
}: {
  context: AgentContext;
}): { target: 'tool' | 'thinking' } => ({
  target: context.pending.length > 0 ? 'tool' : 'thinking',
});
