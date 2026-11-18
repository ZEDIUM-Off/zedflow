import { join } from 'node:path';
import type { ToolDeclaration } from '@zf/core';
import { discoverDeclarations, readDeclaration } from './resource-files.ts';
import { ToolDeclarationDecoder } from './tool-declaration-decoder.ts';

export interface ToolCatalogOptions {
  readonly roots: readonly string[];
  readonly builtins?: readonly ToolDeclaration[];
}
export class ToolCatalog {
  readonly #roots: readonly string[];
  readonly #builtins: readonly ToolDeclaration[];

  public constructor(options: ToolCatalogOptions) {
    this.#roots = [...options.roots];
    this.#builtins = structuredClone(options.builtins ?? []);
    if (new Set(this.#builtins.map((tool) => tool.name)).size !== this.#builtins.length) {
      throw new Error('Duplicate built-in tool names');
    }
  }
  public async list(): Promise<Map<string, ToolDeclaration>> {
    const tools = new Map(this.#builtins.map((tool) => [tool.name, structuredClone(tool)]));
    for (const root of this.#roots) {
      for (const file of await discoverDeclarations(join(root, 'tools'), 'tool.json')) {
        try {
          tools.set(
            file.id,
            new ToolDeclarationDecoder(file.directory, await readDeclaration(file.path)).decode(),
          );
        } catch {
          throw new Error(`Invalid tool declaration: ${file.path}`);
        }
      }
    }
    return tools;
  }
}
