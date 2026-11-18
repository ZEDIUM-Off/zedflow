import { basename } from 'node:path';
import { json } from '@zf/core';
import type { MachineManifest } from '@zf/core';
import { validateSettings } from '@zf/config';
import { confinedEntry, objectValue, validResourceId } from './resource-files.ts';

const allowedKeys = new Set([
  'schemaVersion',
  'name',
  'description',
  'entry',
  'defaults',
  'configSchema',
  'tools',
  'ui',
]);
const typeScriptFile = /\.(?:ts|mts)$/;

/**
 * The `machine.json` value of one machine directory. The decoded manifest is named after the
 * directory, and its entry and UI files are TypeScript files confined to the directory.
 */
export class MachineManifestDecoder {
  readonly #directory: string;
  readonly #value: Record<string, unknown>;

  public constructor(directory: string, value: Record<string, unknown>) {
    this.#directory = directory;
    this.#value = value;
  }

  public async decode(): Promise<MachineManifest> {
    const required = this.#required();
    await confinedEntry(this.#directory, required.entry);
    const defaults =
      this.#value['defaults'] === undefined
        ? undefined
        : validateSettings(this.#value['defaults'], false);
    const configSchema = this.#configSchema();
    const tools = this.#tools();
    const ui = await this.#ui();
    return {
      ...required,
      ...(defaults === undefined ? {} : { defaults }),
      ...(configSchema === undefined ? {} : { configSchema }),
      ...(tools === undefined ? {} : { tools }),
      ...(ui === undefined ? {} : { ui }),
    };
  }

  #required(): Pick<MachineManifest, 'schemaVersion' | 'name' | 'description' | 'entry'> {
    const description = this.#value['description'];
    const entry = this.#value['entry'];
    if (
      Object.keys(this.#value).some((key) => !allowedKeys.has(key)) ||
      this.#value['schemaVersion'] !== 1 ||
      this.#value['name'] !== basename(this.#directory) ||
      typeof description !== 'string' ||
      !description.trim() ||
      typeof entry !== 'string' ||
      !typeScriptFile.test(entry)
    ) {
      throw new Error('Invalid machine declaration');
    }
    return { schemaVersion: 1, name: basename(this.#directory), description, entry };
  }

  #configSchema(): MachineManifest['configSchema'] {
    const configSchema = this.#value['configSchema'];
    return configSchema === undefined
      ? undefined
      : (json(objectValue(json(configSchema))) as NonNullable<MachineManifest['configSchema']>);
  }

  #tools(): MachineManifest['tools'] {
    const tools = this.#value['tools'];
    if (tools === undefined) {
      return undefined;
    }
    if (
      !Array.isArray(tools) ||
      !tools.every(
        (tool: unknown): tool is string => typeof tool === 'string' && validResourceId(tool),
      ) ||
      new Set(tools).size !== tools.length
    ) {
      throw new Error('Invalid machine tool selection');
    }
    return tools;
  }

  async #ui(): Promise<string | undefined> {
    const ui = this.#value['ui'];
    if (ui === undefined) {
      return undefined;
    }
    if (typeof ui !== 'string' || !typeScriptFile.test(ui)) {
      throw new Error('Invalid machine UI entry');
    }
    await confinedEntry(this.#directory, ui);
    return ui;
  }
}
