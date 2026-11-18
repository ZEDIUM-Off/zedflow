import type { AnyStateMachine } from 'xstate';
import { cancelTurn, initial, startPrompt } from './agent-context.ts';
import type { AgentControls } from './agent-controls.ts';
import { agentLogic } from './agent-logic.ts';
import { modelResult, pendingTool } from './agent-results.ts';

/** Native state graph; actors, transitions and counters retain the agent event contract. */
export const agentMachine = (controls: AgentControls, version = '1'): AnyStateMachine =>
  agentLogic(controls).createMachine({
    id: 'zf.agent',
    version,
    context: initial,
    initial: 'ready',
    on: { RESET: { target: '.ready', context: initial } },
    states: {
      ready: { on: { PROMPT: startPrompt } },
      thinking: {
        on: { CANCEL: cancelTurn },
        entry: ({ context }) => ({ context: { ...context, modelCalls: context.modelCalls + 1 } }),
        invoke: {
          src: 'request',
          input: ({ context }) => structuredClone(context.messages),
          onDone: modelResult,
          onError: ({ context, event }) => ({
            target: 'failed',
            context: { ...context, failure: String(event.error), pending: [] },
          }),
        },
      },
      tool: {
        on: { CANCEL: cancelTurn },
        entry: ({ context }) => ({ context: { ...context, toolCalls: context.toolCalls + 1 } }),
        invoke: {
          src: 'execute',
          input: pendingTool,
          onDone: ({ context, event }) => ({
            target: 'thinking',
            context: {
              ...context,
              messages: [...context.messages, structuredClone(event.output)],
              pending: context.pending.slice(1),
            },
          }),
        },
      },
      failed: { on: { PROMPT: startPrompt } },
      cancelled: { on: { PROMPT: startPrompt } },
    },
  });
