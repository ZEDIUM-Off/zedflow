import type { AssistantMessage, Message, ToolCall, ToolResultMessage } from '@earendil-works/pi-ai';
import { createAsyncLogic, setup, types } from 'xstate';
import type { AgentContext } from './agent-context.ts';
import type { AgentControls } from './agent-controls.ts';

export const agentLogic = (controls: AgentControls) => {
  const request = createAsyncLogic({
    schemas: { input: types<Message[]>(), output: types<AssistantMessage>() },
    run: ({ input, signal }) => controls.respond(input, signal),
  });
  const execute = createAsyncLogic({
    schemas: { input: types<ToolCall>(), output: types<ToolResultMessage>() },
    run: ({ input, signal }) => controls.run(input, signal),
  });
  return setup({
    schemas: {
      context: types<AgentContext>(),
      events: {
        PROMPT: types<{ text: string }>(),
        CANCEL: types<Record<never, never>>(),
        RESET: types<Record<never, never>>(),
      },
    },
    actors: { request, execute },
  });
};
