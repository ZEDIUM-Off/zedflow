import { mkdir, mkdtemp } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool } from '@zf/core';

export const fixture = async (): Promise<string> => {
  const root = resolve('.artifacts/codemode-tests');
  await mkdir(root, { recursive: true });
  return mkdtemp(join(root, 'case-'));
};

export const execute = (
  tool: ExecutableTool,
  code: string,
  signal: AbortSignal = new AbortController().signal,
  onUpdate?: (result: ToolResultMessage) => void,
): Promise<ToolResultMessage> =>
  tool.execute({
    call: { type: 'toolCall', id: 'script', name: 'codemode', arguments: { code } },
    signal,
    ...(onUpdate ? { onUpdate } : {}),
  });

export const text = (result: ToolResultMessage): string =>
  result.content
    .filter((item) => item.type === 'text')
    .map((item) => item.text)
    .join('\n');

export const png =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AP8AAP8FAAH/+lyI0QAAAABJRU5ErkJggg==';
