import { basename, isAbsolute, resolve } from 'node:path';
import { json } from '@zf/core';
import type { ToolDeclaration } from '@zf/core';
import { objectValue } from './resource-files.ts';

const allowedKeys = new Set(['schemaVersion', 'name', 'description', 'inputSchema', 'execution']);
const executionKeys = ['command', 'args'];

const isArgument = (value: unknown): value is string =>
  typeof value === 'string' && !value.includes('\0');

/**
 * The `tool.json` value of one tool directory. The decoded declaration is named after the
 * directory and runs a shell-free command whose relative path resolves against the directory.
 */
export class ToolDeclarationDecoder {
  readonly #directory: string;
  readonly #value: Record<string, unknown>;

  public constructor(directory: string, value: Record<string, unknown>) {
    this.#directory = directory;
    this.#value = value;
  }

  public decode(): ToolDeclaration {
    const name = basename(this.#directory);
    const execution = objectValue(this.#value['execution']);
    const inputSchema = objectValue(this.#value['inputSchema']);
    const description = this.#value['description'];
    if (!this.#hasHeader(name) || (description !== undefined && typeof description !== 'string')) {
      throw new Error('Invalid tool declaration');
    }
    return {
      name,
      inputSchema: json(inputSchema),
      execution: this.#execution(execution),
      ...(description === undefined ? {} : { description }),
    };
  }

  #hasHeader(name: string): boolean {
    const schemaVersion = this.#value['schemaVersion'];
    return (
      Object.keys(this.#value).every((key) => allowedKeys.has(key)) &&
      (schemaVersion === undefined || schemaVersion === 1) &&
      this.#value['name'] === name
    );
  }

  #execution(execution: Record<string, unknown>): ToolDeclaration['execution'] {
    const command = execution['command'];
    const args = execution['args'];
    if (
      Object.keys(execution).some((key) => !executionKeys.includes(key)) ||
      !isArgument(command) ||
      !command.trim() ||
      !Array.isArray(args) ||
      !args.every(isArgument)
    ) {
      throw new Error('Invalid tool declaration');
    }
    return {
      command:
        /[\\/]/.test(command) && !isAbsolute(command) ? resolve(this.#directory, command) : command,
      args,
    };
  }
}
