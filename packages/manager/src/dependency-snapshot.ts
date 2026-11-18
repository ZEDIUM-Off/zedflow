import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { Json } from '@zf/core';
import { atomicJson, digestTree } from './files.ts';
import type { InstallerProcess } from './process.ts';
import type { LockedSource } from './types.ts';

/** The isolated dependency project for one prepared source, including its frozen-lock policy. */
export class DependencySnapshot {
  readonly #directory: string;
  readonly #process: InstallerProcess;
  readonly #frozen: LockedSource | undefined;

  public constructor(
    directory: string,
    process: InstallerProcess,
    frozen: LockedSource | undefined,
  ) {
    this.#directory = directory;
    this.#process = process;
    this.#frozen = frozen;
  }

  public get nodeModules(): string {
    return join(this.#directory, 'node_modules');
  }

  public async install(manifest: Readonly<Record<string, Json>>): Promise<void> {
    await mkdir(this.#directory);
    await atomicJson(join(this.#directory, 'package.json'), manifest);
    if (this.#frozen) {
      if (this.#frozen.dependencyLock === null) {
        throw new Error('Frozen dependency lock is missing');
      }
      await writeFile(join(this.#directory, 'pnpm-lock.yaml'), this.#frozen.dependencyLock);
    }
    await this.#process.install(this.#directory, this.#frozen !== undefined);
  }

  public async lock(): Promise<string> {
    return readFile(join(this.#directory, 'pnpm-lock.yaml'), 'utf8');
  }

  public async integrity(): Promise<string> {
    return digestTree({ root: this.nodeModules, links: 'dependencies' });
  }
}
