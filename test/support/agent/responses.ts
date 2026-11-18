import { createAssistantMessageEventStream } from '@earendil-works/pi-ai';
import type { AssistantMessage, Provider, ToolCall } from '@earendil-works/pi-ai';
import { message } from './messages.ts';
export const response =
  (
    content: AssistantMessage['content'],
    reason: Exclude<AssistantMessage['stopReason'], 'pending'> = 'stop',
  ): Provider['streamSimple'] =>
  (model) => {
    const stream = createAssistantMessageEventStream();
    const result = message(model, content, reason);
    stream.push({ type: 'start', partial: result });
    content.forEach((block, contentIndex) => {
      if (block.type === 'text') {
        stream.push({ type: 'text_delta', contentIndex, delta: block.text, partial: result });
      }
      if (block.type === 'toolCall') {
        stream.push({ type: 'toolcall_end', contentIndex, toolCall: block, partial: result });
      }
    });
    if (reason === 'error' || reason === 'aborted') {
      result.errorMessage = `fixture model ${reason}`;
      stream.push({ type: 'error', reason, error: result });
    } else {
      stream.push({ type: 'done', reason, message: result });
    }
    return stream;
  };
export const answer = (text = 'fixture answer'): Provider['streamSimple'] =>
  response([{ type: 'text', text }]);
export const calls = (...items: ToolCall[]): Provider['streamSimple'] => response(items, 'toolUse');
