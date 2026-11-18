import { readdir } from 'node:fs/promises';
import { join } from 'node:path';
import type { Json } from '@zf/core';
import { exists } from './files.ts';
import { jsonObject, readJson } from './json-values.ts';
import { registryDependencies } from './registry-package.ts';

const EXECUTION_CONFIGURATION = [
  '.npmrc',
  '.pnpmfile.cjs',
  'pnpmfile.cjs',
  '.pnpmfile.mjs',
  'pnpmfile.mjs',
  'pnpm-workspace.yaml',
  'pnpm-workspace.yml',
  '.gitmodules',
];
const EXECUTION_FIELDS = ['pnpm', 'packageManager', 'devEngines', 'workspaces'];
const DEPENDENCY_FIELDS = ['dependencies', 'optionalDependencies'] as const;

/** A manifest in the shape the manager generates; its only extra fields are registry dependencies. */
export const safeDependencyManifest = (value: unknown): boolean =>
  jsonObject(value) &&
  (value['name'] === 'zedflow-managed-source' ||
    value['name'] === 'zedflow-managed-dependencies') &&
  value['version'] === '0.0.0' &&
  value['private'] === true &&
  Object.entries(value).every(
    ([key, child]) =>
      ['name', 'version', 'private'].includes(key) ||
      (['dependencies', 'optionalDependencies'].includes(key) && registryDependencies(child)),
  );

/** Manager-generated dependency projects never ingest source npmrc/workspace/pnpm settings. */
export class DependencyManifestReader {
  readonly #source: string;

  public constructor(source: string) {
    this.#source = source;
  }

  public async read(): Promise<Record<string, Json>> {
    await this.#rejectExecutionConfiguration(this.#source);
    const result: Record<string, Json> = {
      name: 'zedflow-managed-dependencies',
      version: '0.0.0',
      private: true,
    };
    const path = join(this.#source, 'package.json');
    if (!(await exists(path))) {
      return result;
    }
    const value = await readJson(path);
    if (!jsonObject(value)) {
      throw new Error('Invalid source package.json');
    }
    if (EXECUTION_FIELDS.some((key) => value[key] !== undefined)) {
      throw new Error('Source package-manager/workspace execution configuration is not supported');
    }
    return { ...result, ...this.#registryFields(value) };
  }

  async #rejectExecutionConfiguration(directory: string): Promise<void> {
    for (const child of await readdir(directory, { withFileTypes: true })) {
      if (EXECUTION_CONFIGURATION.includes(child.name)) {
        throw new Error(
          `Untrusted package execution configuration is not supported: ${child.name}`,
        );
      }
      if (child.isDirectory()) {
        await this.#rejectExecutionConfiguration(join(directory, child.name));
      }
    }
  }

  #registryFields(manifest: Record<string, Json>): Record<string, Json> {
    const fields: Record<string, Json> = {};
    for (const field of DEPENDENCY_FIELDS) {
      const dependencies = manifest[field];
      if (dependencies !== undefined) {
        if (!registryDependencies(dependencies)) {
          throw new Error('Only registry package dependencies are supported');
        }
        fields[field] = dependencies;
      }
    }
    return fields;
  }
}

export const dependencyManifest = async (source: string): Promise<Record<string, Json>> =>
  new DependencyManifestReader(source).read();
