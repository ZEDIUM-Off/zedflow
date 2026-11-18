import { mkdir, mkdtemp } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import type { JsonObject, ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool } from '@zf/core';

export const fixture = async (): Promise<string> => {
  const root = resolve('.artifacts/tools-tests');
  await mkdir(root, { recursive: true });
  return mkdtemp(join(root, 'case-'));
};

export const invoke = (
  tool: ExecutableTool,
  args: JsonObject,
  signal: AbortSignal = new AbortController().signal,
  onUpdate?: (result: ToolResultMessage) => void,
): Promise<ToolResultMessage> =>
  tool.execute({
    call: { type: 'toolCall', id: 'test-call', name: tool.declaration.name, arguments: args },
    signal,
    ...(onUpdate ? { onUpdate } : {}),
  });

export const text = (result: ToolResultMessage): string =>
  result.content
    .filter((block) => block.type === 'text')
    .map((block) => block.text)
    .join('\n');

export const png =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AP8AAP8FAAH/+lyI0QAAAABJRU5ErkJggg==';
