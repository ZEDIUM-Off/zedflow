import { readModel, modelBitmap, expectReadImage } from '../support/tools/read-model.ts';
import { scriptResults } from '../../packages/tools/src/script-results.ts';
import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { createBuiltinTools } from '../../packages/tools/src/index.ts';
import { createReadTool } from '../../packages/tools/src/read.ts';
import { createWriteTool } from '../../packages/tools/src/write.ts';
import { fixture, invoke, text, png } from '../support/tools/invocation.ts';

test('tools public factory returns native declarations and independently executable tools', async () => {
  const cwd = await fixture();
  const tools = createBuiltinTools({ cwd });
  expect(tools.map((tool) => tool.declaration.name)).toEqual([
    'read',
    'write',
    'edit',
    'bash',
    'grep',
    'find',
    'ls',
    ...(process.platform === 'win32' ? ['powershell'] : []),
  ]);
  const result = await invoke(createWriteTool(cwd), {
    path: 'nested/file.txt',
    content: 'hello\nworld',
  });
  expect(result).toMatchObject({
    role: 'toolResult',
    toolCallId: 'test-call',
    toolName: 'write',
    isError: false,
  });
  expect(result.timestamp).toBeGreaterThan(0);
  expect(result.durationMs).toBeGreaterThanOrEqual(0);
  expect(await readFile(join(cwd, 'nested/file.txt'), 'utf8')).toBe('hello\nworld');
  expect(
    text(await invoke(createReadTool(cwd), { path: 'nested/file.txt', offset: 2, limit: 1 })),
  ).toBe('world');
});

test('read preserves native image blocks, structured content and content-based MIME detection', async () => {
  const cwd = await fixture();
  await writeFile(join(cwd, 'picture.dat'), Buffer.from(png, 'base64'));
  const result = await invoke(createReadTool(cwd), { path: 'picture.dat' });
  expect(result.isError).toBe(false);
  expect(result.content).toContainEqual({ type: 'image', data: png, mimeType: 'image/png' });
  expect(scriptResults.get(result)).toMatchObject({
    type: 'image',
    data: png,
    mimeType: 'image/png',
  });
  expect(result.details ?? {}).not.toHaveProperty('structuredContent');
});

test('read streams text, applies line offsets, byte/line truncation and reports oversized lines', async () => {
  const cwd = await fixture();
  await writeFile(
    join(cwd, 'many.txt'),
    Array.from({ length: 2200 }, (_, i) => `line ${i + 1}`).join('\n'),
  );
  const truncated = await invoke(createReadTool(cwd), { path: 'many.txt' });
  expect(text(truncated)).toContain('Use offset=2001');
  expect(truncated.details).toMatchObject({ truncation: { truncated: true, outputLines: 2000 } });
  expect(
    text(await invoke(createReadTool(cwd), { path: 'many.txt', offset: 2199, limit: 1 })),
  ).toContain('line 2199');
  await writeFile(join(cwd, 'huge.txt'), 'é'.repeat(30_000));
  expect(text(await invoke(createReadTool(cwd), { path: 'huge.txt' }))).toContain('exceeds');
  expect((await invoke(createReadTool(cwd), { path: 'missing' })).isError).toBe(true);
});

test('read cancels an in-flight streaming operation and validates arguments before touching disk', async () => {
  const cwd = await fixture();
  const controller = new AbortController();
  let opened = false;
  class CancelledChunks implements AsyncIterable<Uint8Array> {
    public async *[Symbol.asyncIterator](): AsyncGenerator<Uint8Array> {
      opened = true;
      controller.abort();
      yield Buffer.from('not shown');
    }
  }
  const tool = createReadTool(cwd, {
    operations: {
      access: async () => {},
      readFile: async () => Buffer.alloc(0),
      createReadStream: () => new CancelledChunks(),
    },
  });
  expect((await invoke(tool, { path: 'stream' }, controller.signal)).isError).toBe(true);
  expect(opened).toBe(true);
  expect((await invoke(createReadTool(cwd), { path: 123 })).isError).toBe(true);
  expect((await invoke(createWriteTool(cwd), { path: 'unwritten' })).isError).toBe(true);
});

test('read resize worker preserves coordinates and returns provider-safe resized images', async () => {
  const cwd = await fixture();
  // A real 2x2 BMP, detected by bytes and converted through the copied Photon path.
  const bmp = Buffer.alloc(70);
  bmp.write('BM');
  bmp.writeUInt32LE(70, 2);
  bmp.writeUInt32LE(54, 10);
  bmp.writeUInt32LE(40, 14);
  bmp.writeInt32LE(2, 18);
  bmp.writeInt32LE(2, 22);
  bmp.writeUInt16LE(1, 26);
  bmp.writeUInt16LE(24, 28);
  bmp.writeUInt32LE(16, 34);
  bmp.fill(255, 54);
  await writeFile(join(cwd, 'image.bin'), bmp);
  const result = await invoke(
    createReadTool(cwd, { resizeOptions: { maxWidth: 1, maxHeight: 1 } }),
    { path: 'image.bin' },
  );
  expect(result.isError).toBe(false);
  expect(result.content.some((item) => item.type === 'image')).toBe(true);
  expect(text(result)).toContain('original 2x2');
  expect(text(result)).toContain('displayed at 1x1');
  expect(text(result)).toContain('converted from image/bmp');
});

test('read observes native model replacement, mutation and resize precedence without changing native or structured results', async () => {
  const cwd = await fixture();
  await writeFile(join(cwd, 'model.bmp'), modelBitmap);
  const model = readModel();
  model.input = ['text'];
  model.inputLimits = { images: { resize: { maxWidth: 32, maxHeight: 32 } } };
  const options: {
    model?: typeof model;
    resizeOptions: { maxWidth: number; maxHeight: number };
  } = { model, resizeOptions: { maxWidth: 48, maxHeight: 48 } };
  const tool = createReadTool(cwd, options);
  const first = await invoke(tool, { path: 'model.bmp' });
  expectReadImage(first, 32, 16, true);
  expect(text(first)).toContain('original 96x48');
  expect(text(first)).toContain('displayed at 32x16');
  const vision = readModel();
  vision.input = ['text', 'image'];
  vision.inputLimits = { images: { resize: { maxWidth: 24, maxHeight: 24 } } };
  options.model = vision;
  expectReadImage(await invoke(tool, { path: 'model.bmp' }), 24, 12, false);
  delete vision.inputLimits;
  expectReadImage(await invoke(tool, { path: 'model.bmp' }), 48, 24, false);
  vision.input = ['text'];
  expectReadImage(await invoke(tool, { path: 'model.bmp' }), 48, 24, true);
  delete options.model;
  expectReadImage(await invoke(tool, { path: 'model.bmp' }), 48, 24, false);
});

test('read retains no-model resizing fallback and disabled resizing while preserving the native text-only note', async () => {
  const cwd = await fixture();
  await writeFile(join(cwd, 'model.bmp'), modelBitmap);
  const model = readModel();
  model.input = ['text'];
  model.inputLimits = { images: { resize: { maxWidth: 32, maxHeight: 32 } } };
  const native = await invoke(createReadTool(cwd), { path: 'model.bmp' });
  expectReadImage(native, 96, 48, false);
  expectReadImage(
    await invoke(createReadTool(cwd, { resizeOptions: { maxWidth: 48, maxHeight: 48 } }), {
      path: 'model.bmp',
    }),
    48,
    24,
    false,
  );
  expectReadImage(
    await invoke(createReadTool(cwd, { model, autoResizeImages: false }), { path: 'model.bmp' }),
    96,
    48,
    true,
  );
  expectReadImage(
    await invoke(
      createReadTool(cwd, { model, autoResizeImages: undefined, resizeOptions: undefined }),
      {
        path: 'model.bmp',
      },
    ),
    32,
    16,
    true,
  );
  expectReadImage(
    await invoke(createReadTool(cwd, { model, autoResizeImages: true }), { path: 'model.bmp' }),
    32,
    16,
    true,
  );
});

test('read observes model changes after access and readFile at the native note and resize observation points', async () => {
  const cwd = await fixture();
  const textOnly = readModel();
  textOnly.input = ['text'];
  textOnly.inputLimits = { images: { resize: { maxWidth: 32, maxHeight: 32 } } };
  const vision = readModel();
  vision.input = ['text', 'image'];
  vision.inputLimits = { images: { resize: { maxWidth: 24, maxHeight: 24 } } };
  const options = {
    model: vision,
    resizeOptions: { maxWidth: 48, maxHeight: 48 },
    operations: {
      access: async () => {
        options.model = textOnly;
      },
      detectImageMimeType: async () => 'image/bmp',
      readFile: async () => {
        options.model = vision;
        return modelBitmap;
      },
    },
  };
  const result = await invoke(createReadTool(cwd, options), { path: 'model.bmp' });
  expectReadImage(result, 24, 12, true);
  expect(text(result)).toContain('displayed at 24x12');
});
