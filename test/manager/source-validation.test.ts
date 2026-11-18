import {
  lstat,
  mkdir,
  readFile,
  readdir,
  realpath,
  rm,
  symlink,
  writeFile,
} from 'node:fs/promises';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';
import { createMachine, requiredReceipt } from '../support/manager/machine-files.ts';
import { ManagerFixture } from '../support/manager/manager-fixture.ts';

describe('MachineManager public entry', () => {
  let fixture: ManagerFixture;
  beforeEach(async () => {
    fixture = await ManagerFixture.create();
  });
  afterEach(async () => {
    await fixture.close();
  });
  it.each([
    '.npmrc',
    '.pnpmfile.cjs',
    'pnpmfile.cjs',
    '.pnpmfile.mjs',
    'pnpmfile.mjs',
    'pnpm-workspace.yaml',
    'pnpm-workspace.yml',
    '.gitmodules',
  ])(
    'refuses untrusted source execution configuration before dependency commands: %s',
    async (name) => {
      const { source, processRunner, manager } = fixture;
      await createMachine({ directory: source });
      await writeFile(join(source, name), "throw new Error('must not execute');");
      await expect(manager.install(source)).rejects.toThrow(/execution configuration/);
      expect(processRunner.requests()).toEqual([]);
      expect(await manager.list()).toEqual([]);
    },
  );

  it.each([
    { packageManager: 'pnpm@0.0.0' },
    { pnpm: { configDependencies: { malicious: '1.0.0' } } },
    { workspaces: ['nested'] },
    { devEngines: { packageManager: { name: 'pnpm', version: '0.0.0' } } },
    { dependencies: { arbitrary: 'git+file:/must-not-read' } },
    { dependencies: { arbitrary: 'file:/must-not-read' } },
  ])('refuses execution and non-registry dependency directives: %j', async (fields) => {
    const { source, processRunner, manager } = fixture;
    await createMachine({ directory: source });
    await writeFile(join(source, 'package.json'), JSON.stringify(fields));
    await expect(manager.install(source)).rejects.toThrow(
      /configuration|registry package dependencies/,
    );
    expect(processRunner.requests()).toEqual([]);
  });

  it.each([
    { schemaVersion: 2 },
    { entry: '../outside.ts' },
    { entry: '/outside.ts' },
    { entry: 'missing.ts' },
    { entry: 'machine.js' },
    { tools: false },
    { defaults: [] },
    { ui: '../outside.ts' },
  ])(
    'validates native manifest/discovery/entry containment without code execution: %j',
    async (overrides) => {
      const { root, source, manager } = fixture;
      await createMachine({ directory: source, overrides });
      await expect(manager.install(source)).rejects.toThrow();
      expect(await manager.list()).toEqual([]);
      await expect(lstat(join(root, 'machines/agent'))).rejects.toThrow();
    },
  );

  it('rejects nested directory/name mismatches, directory symlinks and escaping resource paths', async () => {
    const { directory, root, source, manager } = fixture;
    const packageRoot = join(directory, 'multi');
    await createMachine({ directory: join(packageRoot, 'machines', 'wrong'), name: 'other' });
    await expect(manager.install(packageRoot)).rejects.toThrow(/name must match/);
    await rm(packageRoot, { recursive: true });
    await mkdir(join(packageRoot, 'machines'), { recursive: true });
    await createMachine({ directory: source });
    await symlink(source, join(packageRoot, 'machines', 'agent'), 'dir');
    await expect(manager.install(packageRoot)).rejects.toThrow(/real files\/directories/);
    await rm(packageRoot, { recursive: true });
    await writeFile(join(directory, 'outside.ts'), 'export default {};');
    await rm(join(source, 'machine.ts'));
    await symlink(join(directory, 'outside.ts'), join(source, 'machine.ts'));
    await expect(manager.install(source)).rejects.toThrow(/real files\/directories/);
    await rm(join(source, 'machine.ts'));
    await writeFile(join(source, 'machine.ts'), 'export default {};');
    await mkdir(root, { recursive: true });
    const outside = join(directory, 'outside-machines');
    await mkdir(outside);
    await symlink(outside, join(root, 'machines'), 'dir');
    await expect(manager.install(source)).rejects.toThrow(/Symlink/);
    expect(await readdir(outside)).toEqual([]);
  });

  it('ignores ancestor workspace/npmrc hooks and uses an isolated npmrc instead of the fixture home', async () => {
    const { directory, root, source, processRunner, manager, registry } = fixture;
    const marker = join(directory, 'ancestor-hook-must-not-execute');
    const hook = join(directory, '.pnpmfile.cjs');
    await writeFile(
      hook,
      `require('node:fs').writeFileSync(${JSON.stringify(marker)}, 'bad'); module.exports = {};`,
    );
    await writeFile(
      join(directory, '.pnpmfile.mjs'),
      `import { writeFileSync } from 'node:fs'; writeFileSync(${JSON.stringify(marker)}, 'bad'); export default {};`,
    );
    await writeFile(join(directory, '.npmrc'), `global-pnpmfile=${hook}\npnpmfile=${hook}\n`);
    await writeFile(join(directory, 'home/.npmrc'), `global-pnpmfile=${hook}\n`);
    await writeFile(
      join(directory, 'pnpm-workspace.yaml'),
      `packages:\n  - '**'\nglobalPnpmfile: ${JSON.stringify(hook)}\n`,
    );
    await registry.publish({
      name: 'fixture-helper',
      version: '1.0.0',
      files: { 'index.js': 'export const value = 1;' },
    });
    await createMachine({ directory: source });
    await writeFile(
      join(source, 'package.json'),
      JSON.stringify({ dependencies: { 'fixture-helper': '1.0.0' } }),
    );
    expect(requiredReceipt(await manager.install(source)).id).toBe('agent');
    await expect(lstat(marker)).rejects.toThrow();
    expect(
      processRunner
        .requests()
        .every(
          (request) =>
            request.env['NPM_CONFIG_USERCONFIG'] === join(root, '.manager/runtime/config/empty'),
        ),
    ).toBe(true);
  }, 30000);

  it.each(['home', 'config/empty', 'config/dependency-policy.cjs'])(
    'rejects runtime metadata symlinks instead of reading/writing outside ownership: %s',
    async (name) => {
      const { directory, root, source, processRunner, manager } = fixture;
      await createMachine({ directory: source });
      const initial = await manager.install(source);
      const lock = await readFile(join(root, '.manager/lock.json'), 'utf8');
      const outside = join(directory, 'outside');
      await mkdir(outside);
      await writeFile(join(outside, 'keep'), 'untouched');
      const runtime = join(root, '.manager/runtime');
      await mkdir(join(runtime, name, '..'), { recursive: true });
      await symlink(name === 'home' ? outside : join(outside, 'keep'), join(runtime, name));
      await writeFile(
        join(source, 'package.json'),
        JSON.stringify({ dependencies: { 'fixture-helper': '1.0.0' } }),
      );
      await expect(manager.update('agent')).rejects.toThrow(/Symlink/);
      expect(await manager.list()).toEqual(initial);
      expect(await readFile(join(root, '.manager/lock.json'), 'utf8')).toBe(lock);
      expect(await readFile(join(outside, 'keep'), 'utf8')).toBe('untouched');
      expect(processRunner.requests()).toEqual([]);
    },
  );

  it('fails visibly on malformed/tampered lock data without overwriting it', async () => {
    const { root, source, manager } = fixture;
    await createMachine({ directory: source });
    await manager.install(source);
    const path = join(root, '.manager/lock.json');
    const original: unknown = JSON.parse(await readFile(path, 'utf8'));
    for (const content of [
      '{broken',
      '[]',
      JSON.stringify({ schemaVersion: 1, sources: {}, receipts: [{ id: '../escape' }] }),
      JSON.stringify({
        schemaVersion: 1,
        sources: {},
        receipts: [],
        groupDirectories: ['../escape'],
      }),
    ]) {
      await writeFile(path, content);
      await expect(manager.list()).rejects.toThrow();
      await expect(manager.install(source)).rejects.toThrow();
      expect(await readFile(path, 'utf8')).toBe(content);
    }
    await writeFile(path, JSON.stringify(original));
    for (const id of ['../escape', '/escape', 'team//agent']) {
      await expect(manager.remove(id)).rejects.toThrow(/Invalid machine ID/);
      await expect(manager.update(id)).rejects.toThrow(/Invalid machine ID/);
    }
    for (const invalid of [
      'git:ext::command#main',
      'git:ssh://host/repo#main',
      'git:https://user:password@example.invalid/repo#main',
      'npm:fixture@file:other',
    ]) {
      await expect(manager.install(invalid)).rejects.toThrow();
    }
    expect(await realpath(root)).toBe(root);
  });
});
