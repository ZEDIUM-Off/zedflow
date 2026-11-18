import { chmod, lstat, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';
import { CommittedCleanupError } from '../../packages/manager/src/index.ts';
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
  it('protects user-authored and locally modified destinations and retains the previous lock on conflicts', async () => {
    const { root, source, manager } = fixture;
    await createMachine({ directory: source });
    const userDirectory = join(root, 'machines/agent');
    await mkdir(userDirectory, { recursive: true });
    await writeFile(join(userDirectory, 'user.txt'), 'keep');
    await expect(manager.install(source)).rejects.toThrow(/User-authored/);
    expect(await readFile(join(userDirectory, 'user.txt'), 'utf8')).toBe('keep');
    await expect(manager.remove('agent')).rejects.toThrow(/not manager-owned/);
    await rm(userDirectory, { recursive: true });
    const installed = await manager.install(source);
    const lock = await readFile(join(root, '.manager/lock.json'), 'utf8');
    await expect(manager.install(source)).rejects.toThrow(/conflict/);
    await writeFile(join(userDirectory, 'user.txt'), 'local edit');
    await expect(manager.update('agent')).rejects.toThrow(/Locally modified/);
    await expect(manager.remove('agent')).rejects.toThrow(/Locally modified/);
    await expect(manager.restore({ frozen: true })).rejects.toThrow(/Locally modified/);
    expect(await manager.list()).toEqual(installed);
    expect(await readFile(join(root, '.manager/lock.json'), 'utf8')).toBe(lock);
    expect(await readFile(join(userDirectory, 'user.txt'), 'utf8')).toBe('local edit');
  });

  it('distinguishes committed snapshot-cleanup failures and preserves recovery backups', async () => {
    const { root, source, manager } = fixture;
    await createMachine({ directory: source });
    const initial = requiredReceipt(await manager.install(source));
    const oldSource = join(root, '.manager/snapshots', initial.installId, 'source');
    await chmod(oldSource, 0o555);
    await writeFile(join(source, 'machine.ts'), 'export default { committed: true };');
    try {
      const error: unknown = await manager.update('agent').catch((failure: unknown) => failure);
      expect(error).toBeInstanceOf(CommittedCleanupError);
      if (!(error instanceof CommittedCleanupError)) {
        throw new Error('Expected committed cleanup diagnostic');
      }
      expect(error.committed).toBe(true);
      expect(error.recoveryDirectory.startsWith(join(root, '.manager/staging/'))).toBe(true);
      expect(
        await readFile(join(error.recoveryDirectory, 'old/agent/.source/machine.ts'), 'utf8'),
      ).not.toContain('committed');
      const current = requiredReceipt(await manager.list());
      expect(current.installId).not.toBe(initial.installId);
      expect(
        await readFile(join(root, current.destination, '.source/machine.ts'), 'utf8'),
      ).toContain('committed: true');
      await expect(lstat(join(root, '.manager/operation.lock'))).rejects.toThrow();
    } finally {
      await chmod(oldSource, 0o755);
    }
  });

  it('distinguishes committed lock-release failures with readable receipts and retained recovery files', async () => {
    const { root, processRunner, manager, registry } = fixture;
    await registry.publish({
      name: 'fixture-machine',
      version: '1.0.0',
      files: { 'machine.json': manifest(), 'machine.ts': 'export default {};' },
    });
    processRunner.intercept(async (request) => {
      if (request.command === 'pnpm') {
        await writeFile(
          join(root, '.manager/operation.lock/blocked'),
          'fixture prevents lock release',
        );
      }
    });
    const error: unknown = await manager
      .install('npm:fixture-machine@1.0.0')
      .catch((failure: unknown) => failure);
    expect(error).toBeInstanceOf(CommittedCleanupError);
    if (!(error instanceof CommittedCleanupError)) {
      throw new Error('Expected committed cleanup diagnostic');
    }
    expect(error.committed).toBe(true);
    expect((await lstat(error.recoveryDirectory)).isDirectory()).toBe(true);
    const receipt = requiredReceipt(await manager.list());
    expect(receipt.resolved).toBe('1.0.0');
    expect((await lstat(join(root, receipt.destination))).isDirectory()).toBe(true);
    expect(await readFile(join(root, '.manager/operation.lock/blocked'), 'utf8')).toContain(
      'fixture',
    );
  }, 30000);

  it('refuses same-scope ancestor collisions and source updates that lose an owned identity', async () => {
    const { directory, root, manager } = fixture;
    const packageRoot = join(directory, 'multi');
    await createMachine({ directory: join(packageRoot, 'machines/team/alpha'), name: 'alpha' });
    await createMachine({ directory: join(root, 'machines/team'), name: 'team' });
    await expect(manager.install(packageRoot)).rejects.toThrow(/ancestor conflicts/);
    await rm(join(root, 'machines/team'), { recursive: true });
    const initial = await manager.install(packageRoot);
    await rm(join(packageRoot, 'machines/team/alpha'), { recursive: true });
    await createMachine({ directory: join(packageRoot, 'machines/team/beta'), name: 'beta' });
    await expect(manager.update('team/alpha')).rejects.toThrow(/removed\/renamed/);
    expect(await manager.list()).toEqual(initial);
  });

  it('serializes actual independent processes without lost ownership updates', async () => {
    const { directory, root, manager } = fixture;
    const alpha = join(directory, 'alpha');
    const beta = join(directory, 'beta');
    await createMachine({ directory: alpha, name: 'alpha' });
    await createMachine({ directory: beta, name: 'beta' });
    const entry = pathToFileURL(resolve('packages/manager/src/index.ts')).href;
    await Promise.all(
      [alpha, beta].map((path) =>
        command({
          directory,
          command: process.execPath,
          args: [
            '--input-type=module',
            '-e',
            `import { MachineManager } from ${JSON.stringify(entry)}; await new MachineManager({directory:${JSON.stringify(root)}}).install(${JSON.stringify(path)});`,
          ],
        }),
      ),
    );
    expect((await manager.list()).map((item) => item.id).sort()).toEqual(['alpha', 'beta']);
    expect((await readdir(join(root, '.manager/snapshots'))).length).toBe(2);
    expect(await readdir(join(root, '.manager/staging'))).toEqual([]);
  }, 30000);
});
