import { chmod, lstat, mkdir, readFile, readdir, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';
import { GitRepository } from '../support/manager/git-repository.ts';
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
  it('installs an actual local Git source, pins commits, disables hooks/config and preserves failed updates', async () => {
    const { directory, root, source, processRunner, manager } = fixture;
    await createMachine({ directory: source });
    const git = new GitRepository(source);
    await git.initialize('first');
    const firstCommit = await git.run(['rev-parse', 'HEAD']);
    await git.run(['branch', 'release']);
    const marker = join(directory, 'git-hook-must-not-run');
    await git.installUnsafeHooks(marker);
    const installed = requiredReceipt(
      await manager.install(`git:${pathToFileURL(source).href}#release`),
    );
    expect(installed.resolved).toBe(firstCommit);
    await expect(lstat(marker)).rejects.toThrow();
    await git.run(['config', '--unset', 'core.fsmonitor']);
    await git.writeEntryAndCommit('export default { version: 2 };', 'second');
    const secondCommit = await git.run(['rev-parse', 'HEAD']);
    await git.run(['branch', '--force', 'release', 'HEAD']);
    const before = await readFile(join(root, '.manager/lock.json'), 'utf8');
    processRunner.intercept(async (request) => {
      if (request.command === 'git' && request.args[0] === 'clone') {
        throw new Error('fixture transport failure');
      }
    });
    await expect(manager.update('agent')).rejects.toThrow('fixture transport failure');
    expect(await readFile(join(root, '.manager/lock.json'), 'utf8')).toBe(before);
    expect(await manager.list()).toEqual([installed]);
    processRunner.intercept(async () => {});
    const updated = requiredReceipt(await manager.update('agent'));
    expect(updated.resolved).toBe(secondCommit);
    await rm(join(root, '.manager/snapshots', updated.installId), { recursive: true });
    await rm(join(root, updated.destination), { recursive: true });
    await git.writeEntryAndCommit('export default { version: 3 };', 'third');
    await git.run(['branch', '--force', 'release', 'HEAD']);
    expect(requiredReceipt(await manager.restore({ frozen: true })).resolved).toBe(secondCommit);
    expect(await readFile(join(root, updated.destination, '.source/machine.ts'), 'utf8')).toContain(
      'version: 2',
    );
    await expect(lstat(marker)).rejects.toThrow();
    for (const request of processRunner.requests().filter((item) => item.command === 'git')) {
      expect(request.env['GIT_CONFIG_NOSYSTEM']).toBe('1');
      expect(request.env['GIT_CONFIG_VALUE_0']).toBe(join(root, '.manager/runtime/empty-hooks'));
      expect(request.env['GIT_CONFIG_VALUE_1']).toBe('false');
    }
    await manager.remove('agent');
  }, 30000);

  it('removes new empty groups after rollback and does not advertise removed staging after release failure', async () => {
    const { directory, root, source, processRunner, manager } = fixture;
    await createMachine({ directory: join(source, 'machines/team/alpha'), name: 'alpha' });
    const git = new GitRepository(source);
    await git.initialize('fixture');
    processRunner.intercept(async (request) => {
      if (request.command === 'git' && request.args[0] === 'checkout') {
        await chmod(join(root, '.manager'), 0o555);
      }
    });
    let failure: unknown;
    try {
      failure = await manager.install(`git:${source}#main`).catch((error: unknown) => error);
    } finally {
      await chmod(join(root, '.manager'), 0o755);
    }
    expect(failure).toBeInstanceOf(AggregateError);
    if (!(failure instanceof AggregateError)) {
      throw new Error('Expected publication/release failures');
    }
    expect(failure.errors).toHaveLength(2);
    expect(failure.message).not.toContain('recovery location:');
    expect(failure.message).toContain('staging was removed');
    expect(await readdir(join(root, '.manager/staging'))).toEqual([]);
    await expect(lstat(join(root, 'machines/team'))).rejects.toThrow();
    await rm(join(root, '.manager/operation.lock'), { recursive: true });
    const parent = join(directory, 'team');
    await createMachine({ directory: parent, name: 'team' });
    expect(requiredReceipt(await manager.install(parent)).id).toBe('team');
  }, 30000);

  it('rolls back visible content and snapshots if atomic lock publication fails', async () => {
    const { root, source, processRunner, manager } = fixture;
    await createMachine({ directory: source });
    const git = new GitRepository(source);
    await git.initialize('first');
    const initial = await manager.install(`git:${source}#main`);
    const lock = await readFile(join(root, '.manager/lock.json'), 'utf8');
    await git.writeEntryAndCommit('export default { changed: true };', 'second');
    processRunner.intercept(async (request) => {
      if (request.command === 'git' && request.args[0] === 'checkout') {
        await chmod(join(root, '.manager'), 0o555);
      }
    });
    try {
      await expect(manager.update('agent')).rejects.toThrow();
    } finally {
      await chmod(join(root, '.manager'), 0o755);
    }
    expect(await readFile(join(root, '.manager/lock.json'), 'utf8')).toBe(lock);
    expect(await manager.list()).toEqual(initial);
    expect(await readFile(join(root, 'machines/agent/.source/machine.ts'), 'utf8')).not.toContain(
      'changed',
    );
    expect(await readdir(join(root, '.manager/snapshots'))).toEqual([
      requiredReceipt(initial).installId,
    ]);
  }, 30000);

  it('refuses immediate lock replacement before changing visible resources and does not release the replacement', async () => {
    const { root, source, processRunner, manager } = fixture;
    await createMachine({ directory: source });
    const git = new GitRepository(source);
    await git.initialize('first');
    const initial = await manager.install(`git:${source}#main`);
    const lock = await readFile(join(root, '.manager/lock.json'), 'utf8');
    processRunner.intercept(async (request) => {
      if (request.command === 'git' && request.args[0] === 'checkout') {
        const files = await import('node:fs/promises');
        await files.rename(
          join(root, '.manager/operation.lock'),
          join(root, '.manager/original-lock'),
        );
        await mkdir(join(root, '.manager/operation.lock'));
      }
    });
    await expect(manager.update('agent')).rejects.toThrow(/lock ownership was lost/);
    expect(await readFile(join(root, '.manager/lock.json'), 'utf8')).toBe(lock);
    expect(await manager.list()).toEqual(initial);
    expect((await lstat(join(root, '.manager/operation.lock'))).isDirectory()).toBe(true);
    expect(await readdir(join(root, '.manager/snapshots'))).toEqual([
      requiredReceipt(initial).installId,
    ]);
  }, 30000);
});
