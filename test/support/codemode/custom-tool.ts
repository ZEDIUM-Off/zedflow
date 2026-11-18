import type { JsonObject, ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool } from '@zf/core';

export const customTool = (
  executeArgs: (args: JsonObject, signal: AbortSignal) => Promise<ToolResultMessage>,
): ExecutableTool => ({
  declaration: {
    name: 'custom',
    description: 'A custom test tool',
    parameters: {
      type: 'object',
      properties: { value: { type: 'string' } },
      required: ['value'],
      additionalProperties: false,
    },
  },
  execute: (input) => executeArgs(input.call.arguments, input.signal),
});
