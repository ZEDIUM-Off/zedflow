import type { Message, ToolCall } from '@earendil-works/pi-ai';
import type { MachineBindings } from '@zf/core';
import { setup, types } from 'xstate';
import type { AnyStateMachine } from 'xstate';
import { AgentTools } from './agent-tools.ts';
import { ModelRequest } from './model-request.ts';
import { agentSettings } from './settings.ts';
import { recordFailure, recordResponse, recordResult } from './transitions.ts';
import * as turn from './turn.ts';
import { TurnBudget } from './turn-budget.ts';

export interface AgentContext {
  readonly messages: Message[];
  readonly pending: ToolCall[];
  readonly modelCalls: number;
  readonly toolCalls: number;
  readonly failure: string | null;
}
export type AgentEvent = { type: 'PROMPT'; text: string } | { type: 'CANCEL' } | { type: 'RESET' };

export const createMachine = (bindings: MachineBindings): AnyStateMachine => {
  const settings = agentSettings(bindings);
  const tools = new AgentTools(bindings);
  const request = new ModelRequest(bindings, settings, tools.declarations());
  const budget = new TurnBudget(settings);

  return setup({
    schemas: {
      context: types<AgentContext>(),
      events: {
        PROMPT: types<{ text: string }>(),
        CANCEL: types<Record<never, never>>(),
        RESET: types<Record<never, never>>(),
      },
    },
    actors: { request: request.logic, execute: tools.logic },
  }).createMachine({
    id: 'zf.agent',
    version: '1',
    context: turn.initialContext,
    initial: 'ready',
    on: {
      RESET: { target: '.ready', context: turn.initialContext },
    },
    states: {
      ready: { on: { PROMPT: turn.startPrompt } },
      thinking: {
        on: { CANCEL: turn.cancelTurn },
        entry: turn.countModelCall,
        invoke: {
          src: 'request',
          input: turn.requestMessages,
          onDone: recordResponse,
          onError: recordFailure,
        },
      },
      route: {
        always: ({ context }) => budget.route(context),
      },
      tool: {
        on: { CANCEL: turn.cancelTurn },
        entry: turn.countToolCall,
        invoke: {
          src: 'execute',
          input: turn.nextCall,
          onDone: recordResult,
          onError: recordFailure,
        },
      },
      nextTool: {
        always: turn.nextTool,
      },
      failed: { on: { PROMPT: turn.startPrompt } },
      cancelled: { on: { PROMPT: turn.startPrompt } },
    },
  });
};
