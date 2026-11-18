import { chmod, lstat, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';
import { GitRepository } from '../support/manager/git-repository.ts';
import { createMachine, manifest, requiredReceipt } from '../support/manager/machine-files.ts';
import { command } from '../support/manager/command.ts';
import { ManagerFixture } from '../support/manager/manager-fixture.ts';

describe('MachineManager public entry', () => {
  let fixture: ManagerFixture;
  beforeEach(async () => {
    fixture = await ManagerFixture.create();
  });
  afterEach(async () => {
    await fixture.close();
  });
  it('restores a local dependency snapshot with its actual frozen pnpm lock, refusing source drift', async () => {
    const { root, source, processRunner, manager, registry } = fixture;
    await registry.publish({
      name: 'fixture-helper',
      version: '1.0.0',
      files: { 'index.js': 'export const value = 1;' },
      metadata: { exports: './index.js' },
    });
    await createMachine({ directory: source });
    await writeFile(
      join(source, 'package.json'),
      JSON.stringify({ type: 'module', dependencies: { 'fixture-helper': '^1.0.0' } }),
    );
    const installed = await manager.install(source);
    const receipt = requiredReceipt(installed);
    const snapshot = join(root, '.manager/snapshots', receipt.installId);
    const lock = await readFile(join(root, '.manager/lock.json'), 'utf8');
    expect(lock).toContain('fixture-helper@1.0.0');
    await rm(snapshot, { recursive: true });
    await rm(join(root, receipt.destination), { recursive: true });
    await registry.publish({
      name: 'fixture-helper',
      version: '1.1.0',
      files: { 'index.js': 'export const value = 2;' },
      metadata: { exports: './index.js' },
    });
    expect(await manager.restore({ frozen: true })).toEqual(installed);
    expect(
      processRunner.requests().some((request) => request.args.includes('--frozen-lockfile')),
    ).toBe(true);
    expect(await readFile(join(root, '.manager/lock.json'), 'utf8')).toBe(lock);
    expect(
      await readFile(join(snapshot, 'dependencies/node_modules/fixture-helper/index.js'), 'utf8'),
    ).toBe('export const value = 1;');
    await rm(snapshot, { recursive: true });
    await rm(join(root, receipt.destination), { recursive: true });
    await writeFile(join(source, 'machine.ts'), 'export default { drift: true };');
    await expect(manager.restore({ frozen: true })).rejects.toThrow(/Frozen.*drift/);
    expect(await readFile(join(root, '.manager/lock.json'), 'utf8')).toBe(lock);
    await expect(lstat(join(root, receipt.destination))).rejects.toThrow();
  }, 30000);

  it('restores the same Git commit across different umasks while still protecting executable bits', async () => {
    const { directory, root, source, manager } = fixture;
    await createMachine({ directory: source });
    await writeFile(join(source, 'runner.sh'), '#!/bin/sh\nexit 0\n', { mode: 0o755 });
    const git = new GitRepository(source);
    await git.initialize('fixture');
    const entry = pathToFileURL(resolve('packages/manager/src/index.ts')).href;
    const run = async (operation: string, umask: number) =>
      command({
        directory,
        command: process.execPath,
        args: [
          '--input-type=module',
          '-e',
          `process.umask(${umask}); const { MachineManager } = await import(${JSON.stringify(entry)}); const manager = new MachineManager({directory:${JSON.stringify(root)}}); await manager.${operation};`,
        ],
      });
    await run(`install(${JSON.stringify(`git:${source}#main`)})`, 0o002);
    const initial = await manager.list();
    const receipt = requiredReceipt(initial);
    const snapshot = join(root, '.manager/snapshots', receipt.installId);
    expect((await lstat(join(snapshot, 'source/machine.ts'))).mode & 0o777).toBe(0o664);
    await rm(snapshot, { recursive: true });
    await rm(join(root, receipt.destination), { recursive: true });
    await run('restore({frozen:true})', 0o022);
    expect(await manager.list()).toEqual(initial);
    expect((await lstat(join(snapshot, 'source/machine.ts'))).mode & 0o777).toBe(0o644);
    await chmod(join(snapshot, 'source/runner.sh'), 0o644);
    await expect(manager.restore({ frozen: true })).rejects.toThrow(
      /source snapshot was locally modified/,
    );
  }, 30000);

  it('protects modified shared source and dependency snapshots', async () => {
    const { root, source, manager, registry } = fixture;
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
    const receipt = requiredReceipt(await manager.install(source));
    const snapshot = join(root, '.manager/snapshots', receipt.installId);
    const original = await readFile(join(snapshot, 'source/machine.ts'), 'utf8');
    await writeFile(join(snapshot, 'source/machine.ts'), 'local edit');
    await expect(manager.restore({ frozen: true })).rejects.toThrow(
      /source snapshot was locally modified/,
    );
    await writeFile(join(snapshot, 'source/machine.ts'), original);
    await writeFile(
      join(snapshot, 'dependencies/node_modules/fixture-helper/index.js'),
      'local dependency edit',
    );
    await expect(manager.update('agent')).rejects.toThrow(
      /dependency snapshot was locally modified/,
    );
    await expect(manager.remove('agent')).rejects.toThrow(
      /dependency snapshot was locally modified/,
    );
    expect(await manager.list()).toEqual([receipt]);
  }, 30000);

  it('refuses corrupted registry archives during frozen reconstruction without changing the lock', async () => {
    const { root, manager, registry } = fixture;
    await registry.publish({
      name: 'fixture-machine',
      version: '1.0.0',
      files: { 'machine.json': manifest(), 'machine.ts': 'export default {};' },
    });
    const receipt = requiredReceipt(await manager.install('npm:fixture-machine@1.0.0'));
    const lock = await readFile(join(root, '.manager/lock.json'), 'utf8');
    await rm(join(root, receipt.destination), { recursive: true });
    await rm(join(root, '.manager/snapshots', receipt.installId), { recursive: true });
    await rm(join(root, '.manager/runtime/store'), { recursive: true });
    await rm(join(root, '.manager/runtime/cache'), { recursive: true });
    registry.corruptTarballs();
    await expect(manager.restore({ frozen: true })).rejects.toThrow();
    expect(await readFile(join(root, '.manager/lock.json'), 'utf8')).toBe(lock);
    await expect(lstat(join(root, receipt.destination))).rejects.toThrow();
    expect(await readdir(join(root, '.manager/snapshots'))).toEqual([]);
  }, 30000);

  it('prepares the complete frozen generation before restoring any missing machine', async () => {
    const { directory, root, source, manager } = fixture;
    await createMachine({ directory: source, name: 'alpha' });
    const beta = join(directory, 'beta');
    await createMachine({ directory: beta, name: 'beta' });
    const alphaReceipt = requiredReceipt(await manager.install(source));
    const betaReceipt = requiredReceipt(await manager.install(beta));
    const lock = await readFile(join(root, '.manager/lock.json'), 'utf8');
    await rm(join(root, alphaReceipt.destination), { recursive: true });
    await rm(join(root, betaReceipt.destination), { recursive: true });
    await rm(join(root, '.manager/snapshots', betaReceipt.installId), { recursive: true });
    await writeFile(join(beta, 'machine.ts'), 'export default { drift: true };');
    await expect(manager.restore({ frozen: true })).rejects.toThrow(/Frozen.*drift/);
    await expect(lstat(join(root, alphaReceipt.destination))).rejects.toThrow();
    expect(await readFile(join(root, '.manager/lock.json'), 'utf8')).toBe(lock);
  });
});
