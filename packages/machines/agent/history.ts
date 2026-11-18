import type { Message, ToolCall, ToolResultMessage } from '@earendil-works/pi-ai';
import { json } from '@zf/core';

export const errorText = (error: unknown): string =>
  (error instanceof Error ? error.message : String(error)).slice(0, 2000);

export const toolError = (call: ToolCall, error: unknown): ToolResultMessage => ({
  role: 'toolResult',
  toolCallId: call.id,
  toolName: call.name,
  content: [{ type: 'text', text: errorText(error) }],
  isError: true,
  timestamp: Date.now(),
});

// Persist only JSON-safe messages, after execution (and Code Mode's ephemeral result consumption).
export const historyMessage = <T extends Message>(message: T): T => json(message) as unknown as T;
