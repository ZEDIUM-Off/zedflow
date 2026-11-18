import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { createCodemodeTool } from '../../packages/tools/src/codemode.ts';
import { createReadTool } from '../../packages/tools/src/read.ts';
import { fixture, execute, text, png } from '../support/codemode/invocation.ts';

test('Code Mode preserves multimodal nested results and emits images from actual read', async () => {
  const cwd = await fixture();
  await writeFile(join(cwd, 'picture'), Buffer.from(png, 'base64'));
  const tool = createCodemodeTool({
    tools: [createReadTool(cwd)],
    artifactDirectory: join(cwd, 'output'),
  });
  const result = await execute(
    tool,
    'const picture = await tools.read({path:"picture"}); image(picture); text(picture.note);',
  );
  expect(result.isError).toBe(false);
  expect(result.content).toContainEqual({ type: 'image', data: png, mimeType: 'image/png' });
  expect(result.nestedCalls).toMatchObject({
    calls: [{ name: 'read', status: 'ok' }],
    complete: true,
  });
  expect(JSON.stringify(result.nestedCalls)).not.toContain(png);
});

test('Code Mode truncation persists full text in artifacts while preserving images and nested details', async () => {
  const cwd = await fixture();
  const artifactDirectory = join(cwd, 'output');
  const tool = createCodemodeTool({ tools: [], maxOutputTokens: 10, artifactDirectory });
  const result = await execute(
    tool,
    `// @options: {"max_output_tokens":10000}\ntext("é".repeat(500)); image({type:"image", data:"${png}", mimeType:"image/png"});`,
  );
  expect(result.isError).toBe(false);
  expect(text(result)).toContain('truncated');
  expect(result.content).toContainEqual({ type: 'image', data: png, mimeType: 'image/png' });
  const details = result.details as { fullOutputPath: string };
  expect(details.fullOutputPath.startsWith(`${artifactDirectory}/`)).toBe(true);
  expect(await readFile(details.fullOutputPath, 'utf8')).toBe('é'.repeat(500));
});

test('Code Mode saves image output once, and file-write failures do not erase successful output', async () => {
  const cwd = await fixture();
  const artifactDirectory = join(cwd, 'output');
  const tool = createCodemodeTool({ tools: [], artifactDirectory });
  const result = await execute(
    tool,
    `image({type:"image",data:"${png}",mimeType:"image/png"}); image({type:"image",data:"${png}",mimeType:"image/png"});`,
  );
  expect(result.isError).toBe(false);
  const paths = [...text(result).matchAll(/Image saved to (.+?) \(/g)].map((match) => match[1]);
  expect(paths).toHaveLength(2);
  expect(paths[0]).toBe(paths[1]);
  const path = paths[0];
  if (!path) {
    throw new Error('Expected image path');
  }
  expect(await readFile(path)).toEqual(Buffer.from(png, 'base64'));
  await writeFile(join(cwd, 'blocked'), 'file');
  const blocked = createCodemodeTool({ tools: [], artifactDirectory: join(cwd, 'blocked') });
  const notSaved = await execute(
    blocked,
    `image({type:"image",data:"${png}",mimeType:"image/png"}); return "retained";`,
  );
  expect(notSaved.isError).toBe(false);
  expect(text(notSaved)).toContain('could not be saved');
  expect(text(notSaved)).toContain('retained');
  expect(notSaved.content.some((item) => item.type === 'image')).toBe(true);
});
