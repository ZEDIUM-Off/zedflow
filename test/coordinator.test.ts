import { mkdir, mkdtemp, readFile, realpath, symlink, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { pathToFileURL } from 'node:url';
import { afterEach, expect, test } from 'vite-plus/test';
import ts from 'typescript';
import { Zedflow } from '../src/index.ts';
import { JsonlRunStore } from '@zf/storage';
import { RunCoordinator } from '@zf/run';
import { TypeScriptModuleLoader } from '@zf/capabilities';
import { ToolPrecedenceFixture } from './support/coordinator/tool-precedence-fixture.ts';
import { XStateEngine } from '@zf/xstate';
import type { Engine } from '@zf/core';
const cleanups: (() => void)[] = [];
afterEach(() => {
  for (const cleanup of cleanups.splice(0)) {
    cleanup();
  }
});
const counter = (version = '1', state = 'ready'): string => {
  return `import { createMachine } from 'xstate';
import { step } from './step.ts';
export default createMachine({ id: 'counter', version: '${version}', context: { count: 0, entries: 0 },
 initial: '${state}', states: { ${state}: {
 entry: ({context}) => ({context: {...context, entries: context.entries + 1}}),
 on: { ADD: ({context}) => ({context: {...context, count: context.count + step}}) }
 } } });`;
};
const fixture = async () => {
  const artifacts = resolve('.artifacts/coordinator-prototype');
  await mkdir(artifacts, { recursive: true });
  const base = await mkdtemp(join(artifacts, 'fixture-'));
  await symlink(await realpath('node_modules'), join(base, 'node_modules'), 'dir');
  const globalDirectory = join(base, 'home', '.zedflow');
  const projectDirectory = join(base, 'project');
  const machineDirectory = join(projectDirectory, '.zedflow', 'machines', 'counter');
  await mkdir(machineDirectory, { recursive: true });
  await writeFile(
    join(machineDirectory, 'machine.json'),
    JSON.stringify({
      schemaVersion: 1,
      name: 'counter',
      description: 'Counter fixture',
      entry: 'machine.ts',
    }),
  );
  const path = join(machineDirectory, 'machine.ts');
  const helper = join(machineDirectory, 'step.ts');
  await writeFile(path, counter());
  await writeFile(helper, 'export const step: number = 1;\n');
  const recordsDirectory = join(base, 'records');
  const core = new Zedflow({
    globalDirectory,
    cwd: projectDirectory,
    recordsDirectory,
    authContext: { env: async () => undefined, fileExists: async () => false },
  });
  const launched: string[] = [];
  const launch = async (origin?: string) => {
    const id = await core.launch('counter', origin);
    launched.push(id);
    return id;
  };
  cleanups.push(() => {
    for (const id of launched) {
      core.runs.stop(id);
    }
  });
  return { base, core, launch, path, helper, recordsDirectory, globalDirectory, projectDirectory };
};
test('core launches a native TypeScript machine and records initialization before external input', async () => {
  const f = await fixture();
  const id = await f.launch();
  f.core.runs.send(id, { type: 'ADD' });
  expect(f.core.runs.view(id)).toEqual({
    value: 'ready',
    context: { count: 1, entries: 1 },
    status: 'active',
  });
  const records = f.core.runs.records(id);
  expect(records[0]?.kind).toBe('opened');
  expect(records.findIndex((record) => record.kind === 'observation')).toBeLessThan(
    records.findIndex((record) => record.kind === 'input'),
  );
  expect(new JsonlRunStore(f.recordsDirectory).read(id)).toEqual(records);
});
test('reload refreshes transitive TS imports while retaining run identity and compatible native context', async () => {
  const f = await fixture();
  const id = await f.launch();
  f.core.runs.send(id, { type: 'ADD' });
  await writeFile(f.helper, 'export const step: number = 10;\n');
  await f.core.runs.reload(id);
  f.core.runs.send(id, { type: 'ADD' });
  expect(f.core.runs.view(id)).toEqual({
    value: 'ready',
    context: { count: 11, entries: 1 },
    status: 'active',
  });
  const replacement = f.core.runs.records(id).find((record) => record.kind === 'replacement');
  expect(replacement?.data).toMatchObject({ generation: 2 });
  const data = replacement?.data as {
    revision: string;
    previousRevision: string;
  };
  expect(data.revision).not.toBe(data.previousRevision);
  expect(f.core.runs.records(id).filter((record) => record.kind === 'opened')).toHaveLength(1);
});
test('a new launch sees new code without mutating an already running machine', async () => {
  const f = await fixture();
  const oldId = await f.launch();
  await writeFile(f.helper, 'export const step: number = 8;\n');
  const newId = await f.launch(oldId);
  f.core.runs.send(oldId, { type: 'ADD' });
  f.core.runs.send(newId, { type: 'ADD' });
  expect(f.core.runs.view(oldId)).toMatchObject({ context: { count: 1 } });
  expect(f.core.runs.view(newId)).toMatchObject({ context: { count: 8 } });
  expect(f.core.runs.records(newId)[0]?.data).toMatchObject({ origin: oldId });
});
test.each(['export default {};', 'export default (', counter('2'), counter('1', 'different')])(
  'invalid or incompatible replacement preserves the old execution: %s',
  async (replacement) => {
    const f = await fixture();
    const id = await f.launch();
    await writeFile(f.path, replacement);
    await expect(f.core.runs.reload(id)).rejects.toThrow();
    f.core.runs.send(id, { type: 'ADD' });
    expect(f.core.runs.view(id)).toMatchObject({ context: { count: 1 } });
    expect(f.core.runs.records(id).some((record) => record.kind === 'replacement')).toBe(false);
  },
);
test.each([
  "import {createMachine} from 'xstate'; export default createMachine({id:'busy',version:'1',initial:'waiting',states:{waiting:{after:{60000:{target:'done'}}},done:{type:'final'}}});",
  "import {setup,createMachine} from 'xstate'; const child=createMachine({initial:'waiting',states:{waiting:{}}}); export default setup({actors:{child}}).createMachine({id:'busy',version:'1',initial:'waiting',states:{waiting:{invoke:{src:'child'}}}});",
])(
  'reload refuses active timers and children rather than restarting their work',
  async (machine) => {
    const f = await fixture();
    await writeFile(f.path, machine);
    const id = await f.launch();
    await expect(f.core.runs.reload(id)).rejects.toThrow('child-free, timer-free');
    expect(f.core.runs.view(id)).toMatchObject({ status: 'active' });
  },
);
test('project machines override global machines and invalid names cannot traverse directories', async () => {
  const f = await fixture();
  const directory = join(f.globalDirectory, 'machines', 'counter');
  await mkdir(directory, { recursive: true });
  await writeFile(
    join(directory, 'machine.json'),
    JSON.stringify({
      schemaVersion: 1,
      name: 'counter',
      description: 'Shadowed global counter',
      entry: 'machine.ts',
    }),
  );
  await writeFile(join(directory, 'machine.ts'), "throw new Error('global must not execute');");
  const id = await f.launch();
  expect(f.core.runs.view(id)).toMatchObject({ status: 'active' });
  await expect(f.core.launch('../counter')).rejects.toThrow('Unknown machine');
});
test('built-in tools exist and project declarations win in the application generation', async () => {
  const f = await fixture();
  expect((await f.core.resources.configuration('counter')).settings['tools']).toEqual(
    expect.arrayContaining(['read', 'write', 'edit', 'bash']),
  );
  const tools = await ToolPrecedenceFixture.create(f.globalDirectory, f.projectDirectory);
  const { nativeTools, catalog } = tools;
  expect((await catalog.list()).get('fixture')?.execution).toEqual({
    command: process.execPath,
    args: ['-e', 'process.stdout.write("project-command")'],
  });
  expect((await catalog.list()).get('read')?.execution).toEqual({ builtin: 'read' });
  expect((await f.core.resources.configuration('counter')).settings['tools']).toContain('fixture');
  await writeFile(
    f.path,
    `import {createMachine as native,createAsyncLogic} from 'xstate';
export const createMachine=(bindings)=>{const tool=bindings.tools.find(t=>t.declaration.name==='fixture');if(!tool)throw new Error('missing fixture');
return native({id:'counter',version:'1',context:{result:null},initial:'running',states:{running:{invoke:{src:createAsyncLogic({run:async({signal})=>tool.execute({call:{type:'toolCall',id:'precedence',name:'fixture',arguments:{}},signal})}),onDone:({context,event})=>({target:'ready',context:{...context,result:event.output}})}},ready:{}}});};`,
  );
  const selected = await f.launch();
  await expect
    .poll(() => f.core.runs.view(selected))
    .toMatchObject({
      value: 'ready',
      context: { result: { isError: false, content: [{ type: 'text', text: 'project-command' }] } },
    });
  const read = nativeTools.find((tool) => tool.declaration.name === 'read');
  if (!read) {
    throw new Error('Missing native read tool');
  }
  expect(
    await read.execute({
      call: { type: 'toolCall', id: 'fixture-read', name: 'read', arguments: { path: f.helper } },
      signal: new AbortController().signal,
    }),
  ).toMatchObject({
    role: 'toolResult',
    toolCallId: 'fixture-read',
    isError: false,
    content: [{ type: 'text', text: 'export const step: number = 1;\n' }],
  });
});
test('FS store rejects path traversal and incomplete trailing records', async () => {
  const f = await fixture();
  const store = new JsonlRunStore(f.recordsDirectory);
  expect(() => store.read('../escape')).toThrow('Invalid run ID');
  await writeFile(join(f.recordsDirectory, 'broken.jsonl'), '{"seq":1');
  expect(() => store.read('broken')).toThrow('Incomplete run record');
});
test('coordinator serializes replacement commands without relying on XState internals', async () => {
  const f = await fixture();
  let release: (() => void) | undefined;
  const pending = new Promise<void>((resolvePending) => {
    release = resolvePending;
  });
  let loads = 0;
  const engine: Engine = {
    identify: () => ({ id: 'fake', version: '1', opaque: null }),
    prepare: () => ({
      start: () => {},
      stop: () => {},
      send: () => {},
      view: () => null,
      checkpoint: () => ({ value: 'ready' }),
    }),
  };
  const coordinator = new RunCoordinator({
    loader: {
      load: async () => {
        if (loads++ > 0) {
          await pending;
        }
        return { path: f.path, revision: 'fixture', module: null };
      },
    },
    engine,
    store: new JsonlRunStore(f.recordsDirectory),
  });
  const id = await coordinator.launch(f.path);
  const reloading = coordinator.reload(id);
  expect(() => coordinator.send(id, { type: 'ADD' })).toThrow('Run unavailable');
  expect(() => coordinator.stop(id)).toThrow('Run unavailable');
  await expect(coordinator.reload(id)).rejects.toThrow('Run unavailable');
  release?.();
  await reloading;
  coordinator.stop(id);
});
test('TypeScript checking and runtime transpilation are distinct, not a false runtime type-safety claim', async () => {
  const f = await fixture();
  const invalid = join(f.base, 'invalid.ts');
  await writeFile(invalid, "const value: number = 'not a number'; export default value;");
  const program = ts.createProgram([invalid], { strict: true, noEmit: true, skipLibCheck: true });
  expect(ts.getPreEmitDiagnostics(program).some((diagnostic) => diagnostic.code === 2322)).toBe(
    true,
  );
  expect((await new TypeScriptModuleLoader().load(invalid)).module).toBe('not a number');
});
test('program XState imports stay inside the adapter, while machines use native XState directly', async () => {
  for (const name of [
    'core/src/contracts',
    'core/src/json',
    'run/src/coordinator',
    'storage/src/fs-store',
    'capabilities/src/module-loader',
    'capabilities/src/tool-catalog',
    'capabilities/src/skill-catalog',
  ]) {
    const text = await readFile(resolve('packages', name + '.ts'), 'utf8');
    expect(text).not.toMatch(/(?:from\s*|import\s*\()["']xstate/);
  }
  expect(new XStateEngine()).toHaveProperty('prepare');
});
test("loader shares the engine's XState identity rather than creating an incompatible second runtime", async () => {
  const f = await fixture();
  const engine = new XStateEngine();
  const separate = await new TypeScriptModuleLoader().load(f.path);
  expect(() => engine.identify(separate.module)).toThrow('shared XState instance');
  const shared = await new TypeScriptModuleLoader(engine.modules).load(f.path);
  expect(engine.identify(shared.module).id).toBe('counter');
});
test('direct Node TypeScript execution works outside the Vite test transform', async () => {
  const f = await fixture();
  const code = `
    import {Zedflow} from ${JSON.stringify(pathToFileURL(resolve('src/index.ts')).href)};
    import {writeFile} from 'node:fs/promises';
    const core=new Zedflow({...${JSON.stringify({ globalDirectory: f.globalDirectory, cwd: f.projectDirectory, recordsDirectory: f.recordsDirectory })},authContext:{env:async()=>undefined,fileExists:async()=>false}});
    const id=await core.launch('counter');
    core.runs.send(id,{type:'ADD'});
    await writeFile(${JSON.stringify(f.helper)}, 'export const step: number = 10;');
    await core.runs.reload(id);
    core.runs.send(id,{type:'ADD'});
    console.log(JSON.stringify(core.runs.view(id)));
    core.runs.stop(id);
  `;
  const { stdout } = await promisify(execFile)(
    process.execPath,
    ['--input-type=module', '--eval', code],
    {
      timeout: 10000,
      env: {
        PATH: process.env['PATH'],
        HOME: join(f.base, 'home'),
        USERPROFILE: join(f.base, 'home'),
        TMPDIR: f.base,
      },
    },
  );
  expect(JSON.parse(stdout)).toMatchObject({
    context: { count: 11, entries: 1 },
    status: 'active',
  });
});
test('domain emissions are captured separately from inspection and stopped runs refuse commands', async () => {
  const f = await fixture();
  await writeFile(
    f.path,
    `import {createMachine} from 'xstate';
    export default createMachine({id:'emitter',version:'1',initial:'ready',states:{ready:{on:{PING:(_args,enq)=>{enq.emit({type:'reply',text:'fixture'});}}}}});`,
  );
  const id = await f.launch();
  f.core.runs.send(id, { type: 'PING' });
  expect(JSON.stringify(f.core.runs.records(id))).toContain('"type":"emitted"');
  expect(JSON.stringify(f.core.runs.records(id))).toContain('"text":"fixture"');
  f.core.runs.stop(id);
  expect(() => f.core.runs.send(id, { type: 'PING' })).toThrow('Run unavailable');
  await expect(f.core.runs.reload(id)).rejects.toThrow('Run unavailable');
});
