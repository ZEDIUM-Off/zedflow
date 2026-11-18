import type { AssistantMessage, ToolCall } from '@earendil-works/pi-ai';
import type { AgentContext } from './agent-context.ts';

export const modelResult = ({
  context,
  event,
}: {
  context: AgentContext;
  event: { output: AssistantMessage };
}) => {
  const calls = event.output.content.filter(
    (block): block is ToolCall => block.type === 'toolCall',
  );
  return {
    target: calls.length > 0 ? 'tool' : 'ready',
    context: {
      ...context,
      messages: [...context.messages, structuredClone(event.output)],
      pending: calls,
    },
  };
};
export const pendingTool = ({ context }: { context: AgentContext }): ToolCall => {
  const call = context.pending[0];
  if (!call) {
    throw new Error('Missing pending tool call');
  }
  return call;
};
