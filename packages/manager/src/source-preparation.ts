import { randomUUID } from 'node:crypto';
import { lstat, mkdir, realpath } from 'node:fs/promises';
import { basename, dirname, join, relative, resolve } from 'node:path';
import type { Json } from '@zf/core';
import { DependencySnapshot } from './dependency-snapshot.ts';
import { contained, copySource, digestTree } from './files.ts';
import { GitCheckout } from './git-checkout.ts';
import type { InstallerProcess } from './process.ts';
import type { LockedSource, PreparedSource, Source } from './types.ts';
import { dependencyManifest, discoverMachines, object, readJson } from './validation.ts';

export interface FrozenSource {
  readonly id: string;
  readonly locked: LockedSource;
}
interface AcquiredContent {
  readonly manifest: Readonly<Record<string, Json>>;
  readonly resolved: string;
  readonly dependencyPath: string | null;
}
const EMPTY_DEPENDENCIES =
  'sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855';
const hasDependencies = (manifest: Readonly<Record<string, Json>>): boolean =>
  ['dependencies', 'optionalDependencies'].some((key) => {
    const value = manifest[key];
    return object(value) && Object.keys(value).length > 0;
  });

/** Prepares sources only inside one operation's staging root using its confined installer. */
export class SourcePreparation {
  readonly #directory: string;
  readonly #process: InstallerProcess;
  readonly #staging: string;

  public constructor(directory: string, process: InstallerProcess, staging: string) {
    this.#directory = directory;
    this.#process = process;
    this.#staging = staging;
  }

  public async prepare(source: Source, frozen?: FrozenSource): Promise<PreparedSource> {
    const id = frozen?.id ?? randomUUID();
    const directory = join(this.#staging, id);
    const dependencies = new DependencySnapshot(
      join(directory, 'dependencies'),
      this.#process,
      frozen?.locked,
    );
    await mkdir(directory);
    const content =
      source.kind === 'npm'
        ? await this.#registry(source, directory, dependencies, frozen)
        : await this.#copy(source, directory, dependencies, frozen);
    const locked = await this.#locked(source, directory, content, dependencies);
    if (frozen && JSON.stringify(locked) !== JSON.stringify(frozen.locked)) {
      throw new Error('Frozen source/dependency integrity drift');
    }
    const machines = await discoverMachines(join(directory, 'source'));
    return { installId: id, directory, locked, machines };
  }

  async #registry(
    source: Source,
    directory: string,
    dependencies: DependencySnapshot,
    frozen?: FrozenSource,
  ): Promise<AcquiredContent> {
    const manifest = frozen?.locked.dependencyManifest ?? {
      name: 'zedflow-managed-source',
      version: '0.0.0',
      private: true,
      dependencies: { [source.location]: source.ref },
    };
    await dependencies.install(manifest);
    const installed = await realpath(join(dependencies.nodeModules, source.location));
    if (!contained(dependencies.nodeModules, installed)) {
      throw new Error('Registry package escaped the dependency snapshot');
    }
    const pkg = await readJson(join(installed, 'package.json'));
    if (!object(pkg) || typeof pkg['version'] !== 'string' || pkg['name'] !== source.location) {
      throw new Error('Invalid resolved registry package');
    }
    await copySource({ from: installed, to: join(directory, 'source') });
    await dependencyManifest(join(directory, 'source'));
    const dependencyRoot = resolve(installed, ...source.location.split('/').map(() => '..'));
    return {
      manifest,
      resolved: pkg['version'],
      dependencyPath: relative(directory, dependencyRoot),
    };
  }

  async #copy(
    source: Source,
    directory: string,
    dependencies: DependencySnapshot,
    frozen?: FrozenSource,
  ): Promise<AcquiredContent> {
    const origin = await this.#origin(source, directory, frozen);
    if (source.kind === 'local' && contained(this.#directory, origin.path)) {
      throw new Error('Manager-owned directories cannot be installed as local sources');
    }
    await copySource({
      from: origin.path,
      to: join(directory, 'source'),
      excludedRoot: this.#directory,
    });
    const manifest = await dependencyManifest(join(directory, 'source'));
    let dependencyPath: string | null = null;
    if (hasDependencies(manifest)) {
      await dependencies.install(manifest);
      dependencyPath = 'dependencies/node_modules';
    }
    return { manifest, resolved: origin.resolved, dependencyPath };
  }

  async #origin(
    source: Source,
    directory: string,
    frozen?: FrozenSource,
  ): Promise<{ path: string; resolved: string }> {
    if (source.kind === 'git') {
      const checkout = join(dirname(directory), `git-${basename(directory)}`);
      const resolved = await new GitCheckout(checkout, this.#process).prepare(
        source,
        frozen?.locked.resolved,
      );
      return { path: checkout, resolved };
    }
    const path = await realpath(source.location);
    if (!(await lstat(path)).isDirectory()) {
      throw new Error('Local source must be a real directory');
    }
    return { path, resolved: '' };
  }

  async #locked(
    source: Source,
    directory: string,
    content: AcquiredContent,
    dependencies: DependencySnapshot,
  ): Promise<LockedSource> {
    const dependencyLock = content.dependencyPath === null ? null : await dependencies.lock();
    const dependencyIntegrity =
      content.dependencyPath === null ? EMPTY_DEPENDENCIES : await dependencies.integrity();
    const sourceIntegrity = await digestTree({ root: join(directory, 'source') });
    return {
      source,
      resolved: source.kind === 'local' ? sourceIntegrity : content.resolved,
      sourceIntegrity,
      dependencyIntegrity,
      dependencyManifest: content.manifest,
      dependencyLock,
      dependencyPath: content.dependencyPath,
    };
  }
}
