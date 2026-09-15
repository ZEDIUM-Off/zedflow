import { test } from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { runInNewContext } from 'node:vm';
import { build } from 'esbuild';

const require = createRequire(import.meta.url);

test('built SDK is consumed outside its workspace by Node, TypeScript and a browser bundle', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'zedflow-sdk-consumer-'));
  try {
    const installed = join(directory, 'node_modules/@zedflow/sdk');
    mkdirSync(installed, { recursive: true });
    // Only distributable files are available to the consumer: no src/ or TS aliases.
    cpSync(resolve('dist'), join(installed, 'dist'), { recursive: true });
    cpSync(resolve('package.json'), join(installed, 'package.json'));
    cpSync(dirname(require.resolve('zod/package.json')), join(directory, 'node_modules/zod'), { recursive: true });
    writeFileSync(join(directory, 'package.json'), '{"type":"module"}');
    const source = `import { createClient, jsonObjectSchema } from '@zedflow/sdk';
      globalThis.consume = async () => {
        const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1,
          fetch: async () => new Response('{"future":{"value":42}}') });
        return client.transport.json({ operation:'fixture', method:'GET', path:'data' }, jsonObjectSchema);
      };`;
    writeFileSync(join(directory, 'consumer.mjs'), source);
    const node = execFileSync(process.execPath, ['--input-type=module', '-e', `
      Object.defineProperty(globalThis, 'window', { get() { throw new Error('window accessed'); } });
      Object.defineProperty(globalThis, 'document', { get() { throw new Error('document accessed'); } });
      await import('./consumer.mjs');
      console.log(JSON.stringify(await globalThis.consume()));
    `], { cwd: directory, encoding: 'utf8' });
    assert.deepEqual(JSON.parse(node), { future: { value: 42 } });

    writeFileSync(join(directory, 'consumer.ts'), `import { createClient } from '@zedflow/sdk';
      import { z } from 'zod';
      const client = createClient({baseUrl:'https://fixture.test/api',protocol:1});
      const result = await client.transport.json({operation:'fixture',method:'GET',path:'data'},z.strictObject({id:z.string()}));
      const id: string = result.id;
      // @ts-expect-error The result is derived from the supplied response schema.
      const bad: number = result.id;
    `);
    execFileSync(process.execPath, [require.resolve('typescript/bin/tsc'), '--noEmit', '--strict', '--target', 'ES2022', '--module', 'NodeNext', 'consumer.ts'], { cwd: directory, encoding: 'utf8' });

    const bundle = await build({ absWorkingDir: directory, entryPoints: ['consumer.mjs'], bundle: true, platform: 'browser', format: 'iife', write: false, metafile: true });
    assert.ok(bundle.outputFiles[0]);
    assert.ok(Object.keys(bundle.metafile.inputs).every(path => !path.includes('/src/') && !path.startsWith('node:')));
    const browser: Record<string, unknown> = { URL, Headers, Response, TextDecoder };
    runInNewContext(bundle.outputFiles[0].text, browser);
    const result = await (browser.consume as () => Promise<unknown>)();
    assert.deepEqual(JSON.parse(JSON.stringify(result)), { future: { value: 42 } });
    assert.ok(!readFileSync(join(installed, 'dist/index.js'), 'utf8').includes('node:'));
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
