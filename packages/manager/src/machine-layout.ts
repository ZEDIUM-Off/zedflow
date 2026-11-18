import { mkdir, rmdir } from 'node:fs/promises';
import { dirname, join, relative, resolve } from 'node:path';
import { exists, safePath } from './files.ts';
import type { LockedSource } from './types.ts';

const hasCode = (error: unknown, code: string): boolean =>
  error instanceof Error && 'code' in error && error.code === code;

/** Owns the managed root and the rules for visible, snapshot and grouping paths. */
export class MachineLayout {
  readonly #directory: string;
  public constructor(directory: string) {
    this.#directory = resolve(directory);
  }
  public get directory(): string {
    return this.#directory;
  }
  public get metadata(): string {
    return join(this.#directory, '.manager');
  }
  public get runtime(): string {
    return join(this.metadata, 'runtime');
  }
  public snapshot(id: string): string {
    return join(this.metadata, 'snapshots', id);
  }

  public links(
    id: string,
    source: LockedSource,
    machine: string,
  ): Readonly<Record<string, string>> {
    return source.dependencyPath === null
      ? {}
      : {
          '.source/node_modules': relative(
            join(this.#directory, 'machines', machine, '.source'),
            join(this.snapshot(id), source.dependencyPath),
          ),
        };
  }

  public async target(id: string): Promise<string> {
    const target = join(this.#directory, 'machines', id);
    await safePath(this.#directory, target);
    let ancestor = dirname(target);
    while (ancestor !== this.#directory) {
      if (await exists(join(ancestor, 'machine.json'))) {
        throw new Error(`Machine ancestor conflicts with ${id}`);
      }
      ancestor = dirname(ancestor);
    }
    return target;
  }

  public async ensureGroups(target: string, created: string[]): Promise<void> {
    const root = join(this.#directory, 'machines');
    await mkdir(root, { recursive: true });
    let group = '';
    for (const segment of relative(root, dirname(target)).split('/').filter(Boolean)) {
      group = group ? `${group}/${segment}` : segment;
      const path = join(root, group);
      await safePath(this.#directory, path);
      try {
        await mkdir(path);
        created.push(group);
      } catch (error) {
        if (!(error instanceof Error && 'code' in error && error.code === 'EEXIST')) {
          throw error;
        }
      }
    }
  }

  public async pruneGroups(groups: Set<string>, removed: string[]): Promise<void> {
    for (const group of [...groups].sort(
      (left, right) => right.split('/').length - left.split('/').length,
    )) {
      const path = join(this.#directory, 'machines', group);
      await safePath(this.#directory, path);
      try {
        // Only directories this manager created are candidates; rmdir never deletes user entries.
        await rmdir(path);
        removed.push(group);
        groups.delete(group);
      } catch (error) {
        if (hasCode(error, 'ENOENT')) {
          groups.delete(group);
        } else if (!hasCode(error, 'ENOTEMPTY') && !hasCode(error, 'EEXIST')) {
          throw error;
        }
      }
    }
  }
}
