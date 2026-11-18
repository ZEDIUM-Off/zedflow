import type { ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool } from '@zf/core';
import { customTool } from './custom-tool.ts';
import { png } from './invocation.ts';

export const richResult = (payload: string): ToolResultMessage => ({
  role: 'toolResult',
  toolCallId: 'nested',
  toolName: 'custom',
  timestamp: 1,
  isError: false,
  content: [
    { type: 'text', text: payload },
    { type: 'image', data: png, mimeType: 'image/png' },
  ],
  details: { payload },
});

export const streamingTool = (rich: ToolResultMessage): ExecutableTool => ({
  declaration: customTool(async () => rich).declaration,
  execute: async (input) => {
    for (let i = 0; i < 2000; i++) {
      input.onUpdate?.(rich);
    }
    return rich;
  },
});
