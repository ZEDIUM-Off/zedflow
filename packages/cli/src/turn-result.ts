import type { Json } from '@zf/core';
import { object } from './command-json.ts';
export const finalText = (view: Json): string => {
  if (!object(view)) {
    return JSON.stringify(view);
  }
  const context = view['context'] ?? null;
  if (!object(context) || !Array.isArray(context['messages'])) {
    return JSON.stringify(view);
  }
  return assistantText(context, context['messages']);
};
const assistantText = (context: { [key: string]: Json }, messages: Json[]): string => {
  const last = messages.findLast((message) => object(message) && message['role'] === 'assistant');
  if (!last || !object(last) || !Array.isArray(last['content'])) {
    return typeof context['failure'] === 'string' ? context['failure'] : '';
  }
  return last['content']
    .map((part) =>
      object(part) && part['type'] === 'text' && typeof part['text'] === 'string'
        ? part['text']
        : '',
    )
    .filter(Boolean)
    .join('\n');
};
export const runFailure = (view: Json): string | undefined => {
  if (!object(view)) {
    return 'Run view is unavailable';
  }
  if (view['status'] === 'error') {
    return 'Native actor failed';
  }
  if (typeof view['error'] === 'string') {
    return 'Run view is unavailable';
  }
  if (view['value'] === 'failed' || view['value'] === 'cancelled') {
    return failureReason(view['context'] ?? null);
  }
  return undefined;
};
const failureReason = (context: Json): string =>
  object(context) && typeof context['failure'] === 'string'
    ? context['failure']
    : 'Agent turn failed';
