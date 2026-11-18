import type { AssistantMessage, ToolCall, ToolResultMessage } from '@earendil-works/pi-ai';
import { errorText } from './history.ts';
import type { AgentContext } from './machine.ts';

export const recordResponse = ({
  context,
  event,
}: {
  context: AgentContext;
  event: { output: AssistantMessage };
}): { target: 'cancelled' | 'route' | 'failed'; context: AgentContext } => ({
  target:
    event.output.stopReason === 'aborted'
      ? 'cancelled'
      : ['stop', 'toolUse'].includes(event.output.stopReason)
        ? 'route'
        : 'failed',
  context: {
    ...context,
    messages: [...context.messages, event.output],
    pending:
      event.output.stopReason === 'toolUse'
        ? event.output.content.filter((block): block is ToolCall => block.type === 'toolCall')
        : [],
    failure: ['stop', 'toolUse'].includes(event.output.stopReason)
      ? null
      : errorText(event.output.errorMessage ?? `Model stopped: ${event.output.stopReason}`),
  },
});

export const recordResult = ({
  context,
  event,
}: {
  context: AgentContext;
  event: { output: ToolResultMessage };
}): { target: 'nextTool'; context: AgentContext } => ({
  target: 'nextTool',
  context: {
    ...context,
    messages: [...context.messages, event.output],
    pending: context.pending.slice(1),
  },
});

export const recordFailure = ({
  context,
  event,
}: {
  context: AgentContext;
  event: { error: unknown };
}): { target: 'failed'; context: AgentContext } => ({
  target: 'failed',
  context: { ...context, failure: errorText(event.error), pending: [] },
});
