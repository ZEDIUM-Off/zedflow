import { chmod, mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { GitRepository } from './git-repository.ts';
import type { RegistryFixture } from './registry-fixture.ts';

type DependencyKind = 'git' | 'tarball' | 'file' | 'link' | 'workspace';

// Owns the hostile leaf, including its deliberately unreadable manifest or Git history.
export class TransitiveDependency {
  readonly #leaf: string;
  readonly #marker: string;
  readonly #kind: DependencyKind;

  public constructor(directory: string, kind: DependencyKind) {
    this.#leaf = join(directory, 'transitive-leaf');
    this.#marker = join(directory, 'transitive-script-must-not-execute');
    this.#kind = kind;
  }

  public get marker(): string {
    return this.#marker;
  }

  public async prepare(registry: RegistryFixture): Promise<string> {
    await mkdir(this.#leaf);
    await writeFile(
      join(this.#leaf, 'package.json'),
      JSON.stringify({
        name: 'fixture-leaf',
        version: '1.0.0',
        scripts: { prepare: `touch ${this.#marker}`, postinstall: `touch ${this.#marker}` },
      }),
    );
    await writeFile(join(this.#leaf, 'index.js'), 'export const value = 1;');
    return this.#range(registry);
  }

  async #range(registry: RegistryFixture): Promise<string> {
    if (this.#kind === 'git') {
      await new GitRepository(this.#leaf).initialize('fixture');
      return `git+${pathToFileURL(this.#leaf).href}#main`;
    }
    if (this.#kind === 'file' || this.#kind === 'link') {
      // Resolution must reject by policy, before an attempted local read would fail with EACCES.
      await chmod(join(this.#leaf, 'package.json'), 0o000);
      return `${this.#kind}:${this.#leaf}`;
    }
    if (this.#kind === 'workspace') {
      return 'workspace:*';
    }
    await registry.publish({
      name: 'fixture-leaf',
      version: '1.0.0',
      files: { 'index.js': 'export const value = 1;' },
    });
    return `${registry.url()}fixture-leaf/-/1.0.0.tgz`;
  }
}
