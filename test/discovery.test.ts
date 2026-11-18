import { mkdir, mkdtemp, readFile, realpath, symlink, writeFile } from 'node:fs/promises';
import { basename, join, resolve } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { createModels, InMemoryCredentialStore } from '@earendil-works/pi-ai';
import { ModuleCatalog, ToolCatalog, TypeScriptModuleLoader } from '@zf/capabilities';
import { BUILTIN_MACHINES_DIRECTORY } from '@zf/machines';
import { XStateEngine } from '@zf/xstate';
import type { MachineBindings, MachineManifest } from '@zf/core';

const definition = `import {createMachine} from 'xstate'; export default createMachine({id:'fixture',version:'1',context:{count:0},initial:'ready',states:{ready:{}}});`;
const fixture = async () => {
  const parent = resolve('.artifacts/cli-goal/discovery-tests');
  await mkdir(parent, { recursive: true });
  const base = await mkdtemp(join(parent, 'case-'));
  await symlink(await realpath('node_modules'), join(base, 'node_modules'), 'dir');
  const roots = [
    { directory: join(base, 'builtin'), scope: 'builtin' as const },
    { directory: join(base, 'global'), scope: 'global' as const },
    { directory: join(base, 'project'), scope: 'project' as const },
  ];
  const engine = new XStateEngine();
  const bindings: MachineBindings = {
    cwd: base,
    config: { marker: 'native-bindings' },
    models: createModels({ credentials: new InMemoryCredentialStore() }),
    tools: [],
  };
  const catalog = new ModuleCatalog({
    roots,
    loader: new TypeScriptModuleLoader(engine.modules),
    bindings: async () => bindings,
  });
  return { base, roots, engine, catalog };
};
const machine = async (root: string, id: string, overrides: Partial<MachineManifest> = {}) => {
  const directory = join(root, id);
  await mkdir(directory, { recursive: true });
  await writeFile(join(directory, 'machine.ts'), definition);
  await writeFile(
    join(directory, 'machine.json'),
    JSON.stringify({
      schemaVersion: 1,
      name: basename(id),
      description: 'Fixture',
      entry: './machine.ts',
      ...overrides,
    }),
  );
  return directory;
};

test('recursive qualified IDs and scope priority are independent of root order, without leaf aliases', async () => {
  const f = await fixture();
  for (const root of f.roots) {
    await machine(root.directory, 'domain/sample', { description: root.scope });
  }
  const catalog = new ModuleCatalog({
    roots: [...f.roots].reverse(),
    loader: new TypeScriptModuleLoader(f.engine.modules),
  });
  expect([...(await catalog.list()).keys()]).toEqual(['domain/sample']);
  expect((await catalog.list()).get('domain/sample')).toMatchObject({
    scope: 'project',
    manifest: { description: 'project' },
  });
  expect(await catalog.resolve('domain/sample')).toBe('machine:domain/sample');
  await expect(catalog.resolve('sample')).rejects.toThrow('Unknown machine');
  const global = new ModuleCatalog({
    roots: f.roots.slice(0, 2),
    loader: new TypeScriptModuleLoader(),
  });
  expect((await global.list()).get('domain/sample')?.scope).toBe('global');
});

test('listing does not execute entries and stops at declared leaves including private source closures', async () => {
  const f = await fixture();
  const directory = await machine(join(f.base, 'project'), 'sample');
  await writeFile(
    join(directory, 'machine.ts'),
    "throw new Error('must not execute during discovery');",
  );
  await machine(join(directory, '.source', 'machines'), 'hidden');
  await machine(directory, 'also-not-a-resource');
  expect([...(await f.catalog.list()).keys()]).toEqual(['sample']);
  await expect(f.catalog.load(await f.catalog.resolve('sample'))).rejects.toThrow(
    'must not execute',
  );
});

test('same-scope conflicts are explicit rather than silently resolved by scan order', async () => {
  const f = await fixture();
  const a = join(f.base, 'a');
  const b = join(f.base, 'b');
  await machine(a, 'nested/sample');
  await machine(b, 'nested/sample');
  const catalog = new ModuleCatalog({
    roots: [
      { directory: a, scope: 'project' },
      { directory: b, scope: 'project' },
    ],
    loader: new TypeScriptModuleLoader(),
  });
  await expect(catalog.list()).rejects.toThrow('Duplicate project machine ID: nested/sample');
});

test.each([
  '../sample',
  '/sample',
  'nested//sample',
  'nested/../sample',
  'Sample',
  'nested\\sample',
  'constructor',
])('invalid identity %s is rejected', async (id) => {
  const f = await fixture();
  await expect(f.catalog.resolve(id)).rejects.toThrow('Invalid machine name');
});

test.each([
  { schemaVersion: 2 },
  { name: 'mismatch' },
  { description: '' },
  { entry: '../outside.ts' },
  { entry: '/absolute.ts' },
  { entry: 'C:\\outside.ts' },
  { entry: 'machine.js' },
  { entry: 'missing.ts' },
  { defaults: { apiKey: 'private-fixture' } },
  { configSchema: 7 },
  { tools: ['read', 'read'] },
  { tools: ['../read'] },
  { ui: '../ui.ts' },
  { unknown: true },
])('invalid declarations fail before loading: %j', async (override) => {
  const f = await fixture();
  const directory = await machine(join(f.base, 'project'), 'sample');
  const manifest = JSON.parse(await readFile(join(directory, 'machine.json'), 'utf8')) as Record<
    string,
    unknown
  >;
  await writeFile(join(directory, 'machine.json'), JSON.stringify({ ...manifest, ...override }));
  await expect(f.catalog.list()).rejects.toThrow('Invalid machine declaration');
});

test('malformed and oversized declarations have bounded non-echoing diagnostics', async () => {
  const f = await fixture();
  const directory = await machine(join(f.base, 'project'), 'sample');
  const path = join(directory, 'machine.json');
  await writeFile(path, '{"secret":"sensitive-fixture" broken');
  try {
    await f.catalog.list();
    throw new Error('unexpected success');
  } catch (error) {
    expect(String(error)).toContain(path);
    expect(String(error)).not.toContain('sensitive-fixture');
  }
  await writeFile(path, ' '.repeat(65537));
  await expect(f.catalog.list()).rejects.toThrow('64 KiB');
});

test('resource directory, manifest and entry symlinks are refused', async () => {
  const f = await fixture();
  const outside = await machine(join(f.base, 'outside'), 'sample');
  const project = join(f.base, 'project');
  await mkdir(project);
  await symlink(outside, join(project, 'sample'), 'dir');
  await expect(f.catalog.list()).rejects.toThrow('symlinks');
  const second = await fixture();
  const directory = await machine(join(second.base, 'project'), 'sample', { entry: 'linked.ts' });
  await symlink(join(outside, 'machine.ts'), join(directory, 'linked.ts'));
  await expect(second.catalog.list()).rejects.toThrow('symlinks');
  const third = await fixture();
  const linked = join(third.base, 'project', 'sample');
  await mkdir(linked, { recursive: true });
  await symlink(join(outside, 'machine.json'), join(linked, 'machine.json'));
  await expect(third.catalog.list()).rejects.toThrow('regular file');
});

test('named factories receive Pi-native bindings and definitions share the engine identity', async () => {
  const f = await fixture();
  const directory = await machine(join(f.base, 'project'), 'sample');
  await writeFile(
    join(directory, 'machine.ts'),
    `import {createMachine as define} from 'xstate'; export function createMachine(bindings) {return define({id:'bound',version:'1',context:{marker:bindings.config.marker,modelCount:bindings.models.getModels().length},initial:'ready',states:{ready:{}}});}`,
  );
  const source = await f.catalog.load(await f.catalog.resolve('sample'));
  const execution = f.engine.prepare(f.engine.identify(source.module), () => {});
  execution.start();
  expect(execution.view()).toMatchObject({ context: { marker: 'native-bindings', modelCount: 0 } });
  execution.stop();
  const unbound = new ModuleCatalog({
    roots: f.roots,
    loader: new TypeScriptModuleLoader(f.engine.modules),
  });
  await expect(unbound.load('machine:sample')).rejects.toThrow(
    'requires native capability bindings',
  );
});

test('resource revision includes helpers outside nested entry directory and the manifest', async () => {
  const f = await fixture();
  const directory = await machine(join(f.base, 'project'), 'sample', {
    entry: '.source/machines/sample/machine.ts',
  });
  const entry = join(directory, '.source', 'machines', 'sample');
  await mkdir(entry, { recursive: true });
  await writeFile(
    join(entry, 'machine.ts'),
    `import {createMachine} from 'xstate'; import {count} from '../../shared.ts'; export default createMachine({id:'fixture',version:'1',context:{count},initial:'ready',states:{ready:{}}});`,
  );
  const helper = join(directory, '.source', 'shared.ts');
  await writeFile(helper, 'export const count=1;');
  const first = await f.catalog.load('machine:sample');
  await writeFile(helper, 'export const count=2;');
  const second = await f.catalog.load('machine:sample');
  expect(first.revision).not.toBe(second.revision);
  const execution = f.engine.prepare(f.engine.identify(second.module), () => {});
  execution.start();
  expect(execution.view()).toMatchObject({ context: { count: 2 } });
  execution.stop();
  const manifest = JSON.parse(await readFile(join(directory, 'machine.json'), 'utf8')) as Record<
    string,
    unknown
  >;
  await writeFile(
    join(directory, 'machine.json'),
    JSON.stringify({ ...manifest, description: 'changed' }),
  );
  expect((await f.catalog.load('machine:sample')).revision).not.toBe(second.revision);
});

test('manifest changes during factory construction refuse the candidate', async () => {
  const f = await fixture();
  const directory = await machine(join(f.base, 'project'), 'sample');
  const path = join(directory, 'machine.json');
  await writeFile(
    join(directory, 'machine.ts'),
    `import {writeFileSync,readFileSync} from 'node:fs'; import {createMachine as define} from 'xstate'; export function createMachine() {const p=${JSON.stringify(path)};const m=JSON.parse(readFileSync(p,'utf8'));m.description='changed';writeFileSync(p,JSON.stringify(m));return define({id:'fixture',version:'1',initial:'ready',states:{ready:{}}});}`,
  );
  await expect(f.catalog.load('machine:sample')).rejects.toThrow(
    'declaration changed while loading',
  );
});

test('native non-conversation built-in uses the ordinary manifest loader', async () => {
  const engine = new XStateEngine();
  const catalog = new ModuleCatalog({
    roots: [{ directory: BUILTIN_MACHINES_DIRECTORY, scope: 'builtin' }],
    loader: new TypeScriptModuleLoader(engine.modules),
  });
  expect((await catalog.list()).get('counter')?.scope).toBe('builtin');
  const source = await catalog.load(await catalog.resolve('counter'));
  const execution = engine.prepare(engine.identify(source.module), () => {});
  execution.start();
  execution.send({ type: 'INCREMENT' });
  expect(execution.view()).toMatchObject({ context: { count: 1 }, value: 'ready' });
  execution.stop();
});

test('tool discovery uses qualified paths, preserves distinct same-leaf IDs and local overrides without execution', async () => {
  const f = await fixture();
  for (const scope of ['global', 'project']) {
    for (const id of ['review/check', 'test/check']) {
      const directory = join(f.base, scope, 'tools', id);
      await mkdir(directory, { recursive: true });
      await writeFile(
        join(directory, 'tool.json'),
        JSON.stringify({
          name: 'check',
          description: scope,
          inputSchema: { type: 'object' },
          execution: { command: 'never-execute', args: [scope] },
        }),
      );
    }
  }
  const catalog = new ToolCatalog({ roots: [join(f.base, 'global'), join(f.base, 'project')] });
  expect([...(await catalog.list()).keys()]).toEqual(['review/check', 'test/check']);
  expect((await catalog.list()).get('review/check')).toMatchObject({
    description: 'project',
    execution: { args: ['project'] },
  });
  expect(
    () =>
      new ToolCatalog({
        roots: [],
        builtins: [
          { name: 'x', inputSchema: {}, execution: { builtin: 'x' } },
          { name: 'x', inputSchema: {}, execution: { builtin: 'x' } },
        ],
      }),
  ).toThrow('Duplicate');
});

test('tool discovery rejects schema arrays and linked directories', async () => {
  const f = await fixture();
  const directory = join(f.base, 'project', 'tools', 'sample');
  await mkdir(directory, { recursive: true });
  await writeFile(
    join(directory, 'tool.json'),
    JSON.stringify({
      name: 'sample',
      inputSchema: [],
      execution: { command: 'never-execute', args: [] },
    }),
  );
  const catalog = new ToolCatalog({ roots: [join(f.base, 'project')] });
  await expect(catalog.list()).rejects.toThrow('Invalid tool declaration');
  const g = await fixture();
  const tools = join(g.base, 'project', 'tools');
  await mkdir(tools, { recursive: true });
  await symlink(directory, join(tools, 'sample'), 'dir');
  await expect(new ToolCatalog({ roots: [join(g.base, 'project')] }).list()).rejects.toThrow(
    'symlinks',
  );
});

test.each(['ts', 'cts'])(
  'loaded external %s helpers participate in source revision',
  async (extension) => {
    const f = await fixture();
    const directory = await machine(join(f.base, 'project'), 'domain/sample');
    const helper = join(directory, '..', `helper.${extension}`);
    const source = (count: number) =>
      extension === 'ts' ? `export const count:number=${count};` : `exports.count=${count};`;
    await writeFile(helper, source(1));
    await writeFile(
      join(directory, 'machine.ts'),
      `import {createMachine} from 'xstate'; import {count} from '../helper.${extension}'; export default createMachine({id:'external',version:'1',context:{count},initial:'ready',states:{ready:{}}});`,
    );
    const first = await f.catalog.load('machine:domain/sample');
    await writeFile(helper, source(2));
    const second = await f.catalog.load('machine:domain/sample');
    expect(second.revision).not.toBe(first.revision);
    const execution = f.engine.prepare(f.engine.identify(second.module), () => {});
    execution.start();
    expect(execution.view()).toMatchObject({ context: { count: 2 } });
    execution.stop();
  },
);

test.each(['cjs', 'mjs', 'js', 'json'])(
  'native local %s imports are refused rather than silently cached',
  async (extension) => {
    const f = await fixture();
    const directory = await machine(join(f.base, 'project'), 'native');
    await writeFile(
      join(directory, `helper.${extension}`),
      extension === 'json' ? '{"count":1}' : 'export const count=1;',
    );
    await writeFile(
      join(directory, 'machine.ts'),
      `import './helper.${extension}'; export default {};`,
    );
    await expect(f.catalog.load('machine:native')).rejects.toThrow(
      'Local machine dependencies must use TypeScript',
    );
  },
);

test.each(['dynamic', 'require'])(
  'the %s import path refuses cached local JavaScript',
  async (form) => {
    const f = await fixture();
    const directory = await machine(join(f.base, 'project'), 'forms');
    await writeFile(join(directory, 'helper.cjs'), 'exports.count=1;');
    await writeFile(join(directory, 'bridge.cts'), `module.exports=require('./helper.cjs');`);
    await writeFile(
      join(directory, 'machine.ts'),
      form === 'dynamic'
        ? `await import('./helper.cjs'); export default {};`
        : `import './bridge.cts'; export default {};`,
    );
    await expect(f.catalog.load('machine:forms')).rejects.toThrow(
      'Local machine dependencies must use TypeScript',
    );
  },
);

test('the import guard preserves original source line numbers', async () => {
  const f = await fixture();
  const directory = await machine(join(f.base, 'project'), 'lines');
  await writeFile(
    join(directory, 'machine.ts'),
    'export default {};\nthrow new Error("line probe");\n',
  );
  try {
    await f.catalog.load('machine:lines');
    expect.unreachable();
  } catch (error) {
    expect(error).toBeInstanceOf(Error);
    expect((error as Error).stack).toContain('machine.ts:2:');
  }
});
