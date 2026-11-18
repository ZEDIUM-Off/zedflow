import { lstat, realpath } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import type { Json, MachineManifest } from '@zf/core';
import { contained, relativePath } from './containment.ts';
import { machineId } from './identifiers.ts';
import { jsonObject, readJson } from './json-values.ts';

const isString = (value: unknown): boolean => typeof value === 'string';
const stringList = (value: unknown): boolean => Array.isArray(value) && value.every(isString);
const optional = (value: unknown, valid: (present: unknown) => boolean): boolean =>
  value === undefined || valid(value);

const validIdentity = (value: Record<string, Json>): boolean =>
  value['schemaVersion'] === 1 &&
  typeof value['name'] === 'string' &&
  machineId(value['name']) &&
  !value['name'].includes('/') &&
  typeof value['description'] === 'string';
const validFiles = (value: Record<string, Json>): boolean =>
  typeof value['entry'] === 'string' &&
  /\.(?:ts|mts|cts|tsx)$/.test(value['entry']) &&
  optional(value['ui'], isString);
const validOptions = (value: Record<string, Json>): boolean =>
  optional(value['defaults'], jsonObject) &&
  optional(value['configSchema'], jsonObject) &&
  optional(value['tools'], stringList);
const validManifest = (value: unknown): value is Record<string, Json> =>
  jsonObject(value) && validIdentity(value) && validFiles(value) && validOptions(value);

/** The `machine.json` of one resource directory; its entry and UI files must stay inside it. */
export class MachineManifestFile {
  readonly #directory: string;

  public constructor(directory: string) {
    this.#directory = directory;
  }

  public async read(): Promise<MachineManifest> {
    const value = await readJson(join(this.#directory, 'machine.json'));
    if (!validManifest(value)) {
      throw new Error(`Invalid MachineManifest: ${this.#directory}`);
    }
    for (const path of [value['entry'], value['ui']]) {
      if (typeof path === 'string') {
        await this.#verifyContainedFile(path);
      }
    }
    return value as unknown as MachineManifest;
  }

  async #verifyContainedFile(path: string): Promise<void> {
    if (!relativePath(path) || !contained(this.#directory, resolve(this.#directory, path))) {
      throw new Error(`Machine entry/ui escapes resource: ${path}`);
    }
    const file = join(this.#directory, path);
    if (
      !(await lstat(file)).isFile() ||
      !contained(await realpath(this.#directory), await realpath(file))
    ) {
      throw new Error(`Machine entry/ui must be a contained real file: ${path}`);
    }
  }
}
