import { lstat, mkdir, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';
import { MultiMachinePackage } from '../support/manager/multi-machine-package.ts';
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
  it('lists without creating state and uses hard-private state/prototype methods', async () => {
    const { root, processRunner, manager } = fixture;
    expect(Object.keys(manager)).toEqual([]);
    expect(Object.hasOwn(manager, 'install')).toBe(false);
    expect(await manager.list()).toEqual([]);
    await expect(lstat(root)).rejects.toThrow();
    expect(processRunner.requests()).toEqual([]);
  });

  it('installs a bare local source with receipts without evaluating entry/UI or running scripts', async () => {
    const { directory, root, source, processRunner, manager } = fixture;
    const marker = join(directory, 'must-not-execute');
    await createMachine({
      directory: source,
      entry: `import { writeFileSync } from 'node:fs'; writeFileSync(${JSON.stringify(marker)}, 'unsafe'); export default {};`,
      overrides: { ui: 'ui.ts', defaults: { feature: true } },
    });
    await writeFile(
      join(source, 'ui.ts'),
      `throw new Error('UI must not execute during install');`,
    );
    await writeFile(
      join(source, 'package.json'),
      JSON.stringify({ type: 'module', scripts: { postinstall: `touch ${marker}` } }),
    );
    const receipt = requiredReceipt(await manager.install(source));
    expect(receipt).toMatchObject({
      id: 'agent',
      source: `local:${source}`,
      destination: 'machines/agent',
      resolved: receipt.sourceIntegrity,
    });
    for (const value of [
      receipt.sourceIntegrity,
      receipt.installedIntegrity,
      receipt.dependencyIntegrity,
    ]) {
      expect(value).toMatch(/^sha256:[a-f0-9]{64}$/);
    }
    expect(receipt.sourceIntegrity).not.toBe(receipt.installedIntegrity);
    expect(await manager.list()).toEqual([receipt]);
    await expect(lstat(marker)).rejects.toThrow();
    const target = join(root, receipt.destination);
    expect((await lstat(target)).isDirectory()).toBe(true);
    expect(JSON.parse(await readFile(join(target, 'machine.json'), 'utf8'))).toMatchObject({
      entry: '.source/machine.ts',
      ui: '.source/ui.ts',
      defaults: { feature: true },
    });
    expect(await readFile(join(target, '.source', 'machine.json'), 'utf8')).toBe(
      await readFile(join(source, 'machine.json'), 'utf8'),
    );
    expect(processRunner.requests()).toEqual([]);
    await manager.remove('agent');
    expect(await manager.list()).toEqual([]);
    expect(await readdir(join(root, '.manager', 'snapshots'))).toEqual([]);
    await expect(lstat(target)).rejects.toThrow();
    expect(await readFile(join(source, 'machine.ts'), 'utf8')).toContain('must-not-execute');
  });

  it('preserves multi-machine source/import/dependency closure, updates owned siblings and removes only one', async () => {
    const { directory, root, manager, registry } = fixture;
    const marker = join(directory, 'must-not-run-dependency-script');
    await registry.publish({
      name: 'fixture-helper',
      version: '1.0.0',
      files: { 'index.js': "export const value = 'helper-v1';" },
      metadata: { exports: './index.js', scripts: { postinstall: `touch ${marker}` } },
    });
    const multi = new MultiMachinePackage(directory);
    await multi.create(marker);
    const packageRoot = multi.directory;
    const receipts = await manager.install(`local:${packageRoot}`);
    expect(receipts.map((item) => item.id)).toEqual(['team/alpha', 'team/beta']);
    const first = requiredReceipt(receipts);
    expect(receipts.every((item) => item.installId === first.installId)).toBe(true);
    const installedEntry = join(root, 'machines/team/alpha/.source/machines/team/alpha/machine.ts');
    await expect(lstat(marker)).rejects.toThrow();
    expect(await fixture.importExport(installedEntry, 'result')).toBe('source-v1/helper-v1');
    await registry.publish({
      name: 'fixture-helper',
      version: '1.1.0',
      files: { 'index.js': "export const value = 'helper-v2';" },
      metadata: { exports: './index.js' },
    });
    await multi.writeShared('source-v2');
    const updated = await manager.update('team/alpha');
    expect(updated.map((item) => item.id)).toEqual(['team/alpha', 'team/beta']);
    expect(requiredReceipt(updated).installId).not.toBe(first.installId);
    expect(await fixture.importExport(installedEntry, 'result')).toBe('source-v2/helper-v2');
    const sharedSnapshot = join(root, '.manager/snapshots', requiredReceipt(updated).installId);
    await manager.remove('team/beta');
    expect((await manager.list()).map((item) => item.id)).toEqual(['team/alpha']);
    expect((await lstat(sharedSnapshot)).isDirectory()).toBe(true);
    const lastUpdate = await manager.update('team/alpha');
    expect(lastUpdate.map((item) => item.id)).toEqual(['team/alpha']);
    await expect(lstat(join(root, 'machines/team/beta'))).rejects.toThrow();
    await manager.remove('team/alpha');
    expect(await readdir(join(root, '.manager/snapshots'))).toEqual([]);
    await expect(lstat(join(root, 'machines/team'))).rejects.toThrow();
    const parentSource = join(directory, 'team');
    await createMachine({ directory: parentSource, name: 'team' });
    expect(requiredReceipt(await manager.install(parentSource)).id).toBe('team');
    await expect(lstat(marker)).rejects.toThrow();
  }, 30000);

  it('canonicalizes local sources reached through symlinked parents before self-copy checks', async () => {
    const { directory, root, source, manager } = fixture;
    await createMachine({ directory: source });
    const initial = await manager.install(source);
    const alias = join(directory, 'home-alias');
    await symlink(join(directory, 'home'), alias, 'dir');
    await expect(manager.install(join(alias, '.zedflow'))).rejects.toThrow(
      /Manager-owned directories/,
    );
    await expect(
      manager.install(
        join(alias, '.zedflow/.manager/snapshots', requiredReceipt(initial).installId, 'source'),
      ),
    ).rejects.toThrow(/Manager-owned directories/);
    expect(await manager.list()).toEqual(initial);
    expect(await readdir(join(root, '.manager/staging'))).toEqual([]);
    await createMachine({ directory: join(directory, 'home'), name: 'home-agent' });
    const ancestor = requiredReceipt(await manager.install(alias));
    expect(ancestor.id).toBe('home-agent');
    await expect(lstat(join(root, ancestor.destination, '.source/.zedflow'))).rejects.toThrow();
  });

  it('keeps preexisting grouping directories and user data while pruning only owned empty groups', async () => {
    const { directory, root, manager } = fixture;
    const packageRoot = join(directory, 'multi');
    await createMachine({ directory: join(packageRoot, 'machines/team/alpha'), name: 'alpha' });
    await mkdir(join(root, 'machines/team'), { recursive: true });
    await manager.install(packageRoot);
    await manager.remove('team/alpha');
    expect(await readdir(join(root, 'machines/team'))).toEqual([]);
    await rm(join(root, 'machines/team'), { recursive: true });
    await manager.install(packageRoot);
    await writeFile(join(root, 'machines/team/user.txt'), 'keep grouping data');
    await manager.remove('team/alpha');
    expect(await readFile(join(root, 'machines/team/user.txt'), 'utf8')).toBe('keep grouping data');
    const parent = join(directory, 'team');
    await createMachine({ directory: parent, name: 'team' });
    await expect(manager.install(parent)).rejects.toThrow(/User-authored/);
  });
});
