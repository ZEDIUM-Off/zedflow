import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { createCodemodeTool } from '../../packages/tools/src/codemode.ts';
import { createReadTool } from '../../packages/tools/src/read.ts';
import { createBashTool } from '../../packages/tools/src/bash.ts';
import { fixture, execute, text } from '../support/codemode/invocation.ts';
import { customTool } from '../support/codemode/custom-tool.ts';

test('Code Mode enforces host deadlines and nested call limits even if source requests more', async () => {
  const cwd = await fixture();
  const tool = createCodemodeTool({
    tools: [createBashTool(cwd)],
    timeoutMs: 400,
    maxToolCalls: 1,
  });
  const startedAt = performance.now();
  const timeout = await execute(tool, '// @options: {"timeout_ms":10000}\nwhile(true) {}');
  expect(timeout.isError).toBe(true);
  expect(text(timeout)).toContain('timeout');
  expect(performance.now() - startedAt).toBeLessThan(3000);
  const exhausted = await execute(
    tool,
    'await tools.bash({command:"true"}); await tools.bash({command:"true"});',
  );
  expect(exhausted.isError).toBe(true);
  expect(text(exhausted)).toContain('call limit');
  // A terminated VM cannot poison a later execution.
  expect(text(await execute(tool, 'return 42;'))).toContain('42');
});

test('Code Mode concurrency is bounded and pre-aborted or malformed calls never start a VM', async () => {
  const cwd = await fixture();
  const controller = new AbortController();
  let release: (() => void) | undefined;
  const ready = new Promise<void>((resolveReady) => {
    release = resolveReady;
  });
  const tool = createCodemodeTool({
    tools: [
      customTool(async (_args, signal) => {
        release?.();
        await new Promise<void>((resolveAbort) =>
          signal.addEventListener('abort', () => resolveAbort(), { once: true }),
        );
        throw new Error('aborted');
      }),
    ],
    maxConcurrentExecutions: 1,
  });
  const first = execute(tool, 'await tools.custom({value:"wait"});', controller.signal);
  await ready;
  expect(text(await execute(tool, 'return "not admitted";'))).toContain('concurrency limit');
  controller.abort();
  await first;
  expect((await execute(tool, 'return "admitted";')).isError).toBe(false);
  const aborted = new AbortController();
  aborted.abort();
  expect((await execute(tool, 'while(true) {}', aborted.signal)).isError).toBe(true);
  const malformed = await tool.execute({
    call: { type: 'toolCall', id: 'bad', name: 'codemode', arguments: {} },
    signal: new AbortController().signal,
  });
  expect(malformed.isError).toBe(true);
  expect(() => createCodemodeTool({ tools: [], timeoutMs: Infinity })).toThrow();
  expect(() => createCodemodeTool({ tools: [createReadTool(cwd), createReadTool(cwd)] })).toThrow(
    'Duplicate',
  );
});

test('Code Mode applies source option parsing and bounded QuickJS memory instead of host JavaScript eval', async () => {
  const tool = createCodemodeTool({ tools: [], memoryLimitBytes: 1024 * 1024 });
  const invalid = await execute(tool, '// @options: {"timeout_ms":-1}\nreturn 1;');
  expect(invalid.isError).toBe(true);
  const exhausted = await execute(
    tool,
    'const data = []; while (true) { data.push("x".repeat(10000)); }',
  );
  expect(exhausted.isError).toBe(true);
  expect(text(exhausted)).toMatch(/memory|allocation/);
});

test('cold WASM timeouts cannot start workers after settlement or retain admission slots', async () => {
  const cwd = await fixture();
  const entry = pathToFileURL(resolve('packages/tools/src/codemode.ts')).href;
  const code = `
    import {createCodemodeTool} from ${JSON.stringify(entry)};
    import {setTimeout} from 'node:timers/promises';
    import {createRequire,syncBuiltinESMExports} from 'node:module';
    const require=createRequire(import.meta.url);
    const workers=require('node:worker_threads'), fs=require('node:fs/promises');
    const NativeWorker=workers.Worker, nativeRead=fs.readFile;
    let settled=false, lateWorkers=0;
    workers.Worker=class extends NativeWorker {constructor(...args) {if(settled)lateWorkers++;super(...args);}};
    fs.readFile=async (...args)=>{if(String(args[0]).includes('.wasm'))await setTimeout(80);return nativeRead(...args);};
    syncBuiltinESMExports();
    const tool=createCodemodeTool({tools:[],timeoutMs:1,maxConcurrentExecutions:1,artifactDirectory:${JSON.stringify(cwd)}});
    const run=()=>tool.execute({call:{type:'toolCall',id:'cold',name:'codemode',arguments:{code:'return 1'}},signal:new AbortController().signal});
    const result=await run(); settled=true;
    await setTimeout(300);
    settled=false; const recovered=await run();
    await setTimeout(100);
    console.log(JSON.stringify({result,recovered,lateWorkers,ports:process.getActiveResourcesInfo().filter(x=>x==='MessagePort').length}));
  `;
  const result = await promisify(execFile)(
    process.execPath,
    ['--input-type=module', '--eval', code],
    { cwd, timeout: 5000 },
  );
  const decoded: unknown = JSON.parse(result.stdout);
  expect(decoded).toMatchObject({ ports: 0, lateWorkers: 0, result: { isError: true } });
  expect(result.stdout).not.toContain('concurrency limit');
});
