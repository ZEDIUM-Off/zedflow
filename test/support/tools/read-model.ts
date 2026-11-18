import { createModels, InMemoryCredentialStore } from '@earendil-works/pi-ai';
import type { Api, Model, ToolResultMessage } from '@earendil-works/pi-ai';
import { openaiProvider } from '@earendil-works/pi-ai/providers/openai';
import { expect } from 'vite-plus/test';
import { scriptResults } from '../../../packages/tools/src/script-results.ts';
import { text } from './invocation.ts';

export const readModel = (): Model<Api> => {
  const models = createModels({
    credentials: new InMemoryCredentialStore(),
    authContext: { env: async () => undefined, fileExists: async () => false },
  });
  models.setProvider(openaiProvider());
  const model = models.getModel('openai', 'gpt-6.1-sol');
  if (!model) {
    throw new Error('Pinned native OpenAI catalog is missing gpt-6.1-sol');
  }
  return structuredClone(model);
};

// Solid white 96x48, 24-bit BMP with a 54-byte header and unpadded pixel rows.
export const modelBitmap = Buffer.concat([
  Buffer.from(
    '424d36360000000000003600000028000000600000003000000001001800000000000036000000000000000000000000000000000000',
    'hex',
  ),
  Buffer.alloc(96 * 48 * 3, 255),
]);

const expectedReadNote = (width: number, height: number, nonVision: boolean): string =>
  [
    'Read image file [image/png]',
    '[Image converted from image/bmp to image/png.]',
    ...(width === 96
      ? []
      : [
          `[Image: original 96x48, displayed at ${width}x${height}. Multiply coordinates by ${(96 / width).toFixed(2)} to map to original image.]`,
        ]),
    ...(nonVision
      ? ['[Current model does not support images. The image will be omitted from this request.]']
      : []),
  ].join('\n');

export const expectReadImage = (
  result: ToolResultMessage,
  width: number,
  height: number,
  nonVision: boolean,
): void => {
  const image = result.content.find((block) => block.type === 'image');
  expect(result).toMatchObject({ toolName: 'read', toolCallId: 'test-call', isError: false });
  if (!image) {
    throw new Error('Native read did not return an image block');
  }
  expect(image.mimeType).toBe('image/png');
  const bytes = Buffer.from(image.data, 'base64');
  expect([bytes.readUInt32BE(16), bytes.readUInt32BE(20)]).toEqual([width, height]);
  expect(text(result)).toBe(expectedReadNote(width, height, nonVision));
  expect(scriptResults.get(result)).toEqual({ ...image, note: text(result) });
  expect(result.details ?? {}).not.toHaveProperty('structuredContent');
};
