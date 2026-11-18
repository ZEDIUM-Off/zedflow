import type { Json } from '@zf/core';

export const AGENT_MACHINE_ID = 'agent';

/** Native agent states during which a model or tool call is in flight. */
const ACTIVE_STATES: ReadonlySet<string> = new Set(['thinking', 'tool']);

export interface AgentSnapshot {
  readonly state: string | undefined;
  readonly messages: readonly Json[];
  readonly pending: readonly Json[];
  readonly modelCalls: number | undefined;
  readonly toolCalls: number | undefined;
  readonly failure: string | undefined;
}

const isRecord = (value: Json | undefined): value is { [key: string]: Json } =>
  typeof value === 'object' && value !== null && !Array.isArray(value);
const list = (value: Json | undefined): readonly Json[] => (Array.isArray(value) ? value : []);
const count = (value: Json | undefined): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined;

/** Read the native agent view `{value, context}`; unknown shapes yield an empty snapshot. */
export const readAgentSnapshot = (view: Json): AgentSnapshot => {
  const record = isRecord(view) ? view : {};
  const context = isRecord(record['context']) ? record['context'] : {};
  const state = typeof record['value'] === 'string' ? record['value'] : undefined;
  const failure = context['failure'];
  return {
    state,
    messages: list(context['messages']),
    pending: list(context['pending']),
    modelCalls: count(context['modelCalls']),
    toolCalls: count(context['toolCalls']),
    failure: typeof failure === 'string' ? failure : undefined,
  };
};

export const isAgentActive = (snapshot: AgentSnapshot): boolean =>
  snapshot.state !== undefined && ACTIVE_STATES.has(snapshot.state);
