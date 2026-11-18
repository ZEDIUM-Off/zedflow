import { lstat, readFile, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';
import { scopedMachinePackage } from '../support/manager/scoped-machine-package.ts';
import { TransitiveDependency } from '../support/manager/transitive-dependency.ts';
import { createMachine, manifest, requiredReceipt } from '../support/manager/machine-files.ts';
import { ManagerFixture } from '../support/manager/manager-fixture.ts';

describe('MachineManager public entry', () => {
  let fixture: ManagerFixture;
  beforeEach(async () => {
    fixture = await ManagerFixture.create();
  });
  afterEach(async () => {
    await fixture.close();
  });
  it('installs/updates scoped registry packages with working dependency imports and frozen restoration', async () => {
    const { root, processRunner, manager, registry } = fixture;
    await registry.publish({
      name: 'fixture-helper',
      version: '1.0.0',
      files: { 'index.js': "export const value = 'scoped-dependency';" },
      metadata: { exports: './index.js' },
    });
    await registry.publish(scopedMachinePackage('1.0.0', 1));
    const first = await manager.install('npm:@fixture/machines@^1.0.0');
    const receipt = requiredReceipt(first);
    expect(receipt.resolved).toBe('1.0.0');
    const importDependency = async () =>
      fixture.importExport(join(root, receipt.destination, '.source/machine.ts'), 'value');
    expect(await importDependency()).toBe('scoped-dependency');
    const lock = await readFile(join(root, '.manager/lock.json'), 'utf8');
    await registry.publish(scopedMachinePackage('1.1.0', 2));
    await rm(join(root, '.manager/snapshots', receipt.installId), { recursive: true });
    await rm(join(root, receipt.destination), { recursive: true });
    expect(await manager.restore({ frozen: true })).toEqual(first);
    expect(await importDependency()).toBe('scoped-dependency');
    expect(await readFile(join(root, receipt.destination, '.source/machine.ts'), 'utf8')).toContain(
      'version: 1',
    );
    expect(await readFile(join(root, '.manager/lock.json'), 'utf8')).toBe(lock);
    const updated = requiredReceipt(await manager.update('agent'));
    expect(updated.resolved).toBe('1.1.0');
    expect(await importDependency()).toBe('scoped-dependency');
    expect(registry.requests()).toContain('/@fixture/machines');
    await manager.remove('agent');
    expect(await manager.list()).toEqual([]);
    const pnpmRequests = processRunner.requests().filter((request) => request.command === 'pnpm');
    expect(pnpmRequests.length).toBeGreaterThanOrEqual(3);
    for (const request of pnpmRequests) {
      expect(request.args).toEqual(
        expect.arrayContaining([
          '--ignore-scripts',
          '--ignore-workspace',
          '--config.ignore-pnpmfile=false',
          `--config.pnpmfile=${join(root, '.manager/runtime/config/dependency-policy.cjs')}`,
        ]),
      );
      expect(request.env['HOME']).toBe(join(root, '.manager/runtime/home'));
      expect(request.env['TMPDIR']).toBe(join(root, '.manager/runtime/tmp'));
      expect(request.env['XDG_CACHE_HOME']).toBe(join(root, '.manager/runtime/cache'));
      expect(request.env['GIT_ALLOW_PROTOCOL']).toBe('http:https:file');
      expect(request.env['NODE_OPTIONS']).toBeUndefined();
    }
  }, 30000);

  it('does not execute registry package pnpmfile or lifecycle hooks', async () => {
    const { directory, manager, registry } = fixture;
    const marker = join(directory, 'registry-hook-must-not-run');
    await registry.publish({
      name: 'fixture-machine',
      version: '1.0.0',
      files: {
        'machine.json': manifest(),
        'machine.ts': 'export default {};',
        '.pnpmfile.cjs': `require('node:fs').writeFileSync(${JSON.stringify(marker)}, 'bad'); module.exports = {};`,
      },
      metadata: { scripts: { postinstall: `touch ${marker}` } },
    });
    await expect(manager.install('npm:fixture-machine@1.0.0')).rejects.toThrow(
      /execution configuration/,
    );
    await expect(lstat(marker)).rejects.toThrow();
    expect(await manager.list()).toEqual([]);
  }, 30000);

  it.each(['git', 'tarball', 'file', 'link', 'workspace'] as const)(
    'checks actual pnpm handling of a registry dependency with a transitive %s source',
    async (kind) => {
      const { directory, source, processRunner, manager, registry } = fixture;
      const dependency = new TransitiveDependency(directory, kind);
      const marker = dependency.marker;
      const range = await dependency.prepare(registry);
      await registry.publish({
        name: 'fixture-middle',
        version: '1.0.0',
        files: { 'index.js': 'export const middle = 1;' },
        metadata: { dependencies: { 'fixture-leaf': range } },
      });
      await createMachine({ directory: source });
      await writeFile(
        join(source, 'package.json'),
        JSON.stringify({ dependencies: { 'fixture-middle': '1.0.0' } }),
      );
      const error: unknown = await manager.install(source).catch((failure: unknown) => failure);
      expect(error).toBeInstanceOf(Error);
      if (!(error instanceof Error)) {
        throw new Error('Expected registry dependency policy rejection');
      }
      expect(error.message).toContain('ZF_REGISTRY_ONLY');
      expect(error.message).toContain('fixture-leaf');
      await writeFile(resolve(`.artifacts/manager-review-transitive-${kind}.log`), error.message);
      await expect(lstat(marker)).rejects.toThrow();
      expect(await manager.list()).toEqual([]);
      expect(registry.requests()).not.toContain('/fixture-leaf/-/1.0.0.tgz');
      expect(
        processRunner.requests().find((request) => request.command === 'pnpm')?.args,
      ).toContain('--config.block-exotic-subdeps=true');
    },
    30000,
  );

  it.each(['parent', 'absolute', 'backslash'] as const)(
    'rejects a correctly checksummed registry tarball with %s path traversal',
    async (kind) => {
      const { directory, root, manager, registry } = fixture;
      const sentinel = join(directory, 'tar-escape.txt');
      await writeFile(sentinel, 'preserve outside destination');
      const path =
        kind === 'absolute'
          ? sentinel
          : kind === 'backslash'
            ? 'package/..\\..\\tar-escape.txt'
            : 'package/../tar-escape.txt';
      await registry.publish({
        name: 'fixture-traversal',
        version: '1.0.0',
        files: { 'machine.json': manifest(), 'machine.ts': 'export default {};' },
        archiveEntries: [{ path, content: 'malicious archive overwrite' }],
      });
      const error: unknown = await manager
        .install('npm:fixture-traversal@1.0.0')
        .catch((failure: unknown) => failure);
      expect(error).toBeInstanceOf(Error);
      if (!(error instanceof Error)) {
        throw new Error('Expected archive path rejection');
      }
      expect(error.message).toContain('ERR_PNPM_TARBALL_IO_ERROR');
      expect(error.message).toContain('tar entry path rejected');
      await writeFile(resolve(`.artifacts/manager-review-tar-${kind}.log`), error.message);
      expect(await readFile(sentinel, 'utf8')).toBe('preserve outside destination');
      expect(await manager.list()).toEqual([]);
      expect(registry.requests()).toContain('/fixture-traversal/-/1.0.0.tgz');
      await expect(lstat(join(root, 'machines/agent'))).rejects.toThrow();
    },
    30000,
  );
});
