import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { ToolCatalog } from '@zf/capabilities';
import { json } from '@zf/core';
import type { ExecutableTool } from '@zf/core';
import { createBuiltinTools } from '../../../packages/tools/src/index.ts';

export class ToolPrecedenceFixture {
  readonly #nativeTools: readonly ExecutableTool[];
  readonly #catalog: ToolCatalog;

  private constructor(globalDirectory: string, projectDirectory: string) {
    this.#nativeTools = createBuiltinTools({ cwd: projectDirectory });
    this.#catalog = new ToolCatalog({
      roots: [globalDirectory, join(projectDirectory, '.zedflow')],
      builtins: this.#nativeTools.map((tool) => ({
        name: tool.declaration.name,
        inputSchema: json(tool.declaration.parameters),
        execution: { builtin: tool.declaration.name },
      })),
    });
  }

  public get nativeTools(): readonly ExecutableTool[] {
    return this.#nativeTools;
  }
  public get catalog(): ToolCatalog {
    return this.#catalog;
  }

  public static async create(
    globalDirectory: string,
    projectDirectory: string,
  ): Promise<ToolPrecedenceFixture> {
    for (const [root, command] of [
      [globalDirectory, 'global-command'],
      [join(projectDirectory, '.zedflow'), 'project-command'],
    ] as const) {
      const directory = join(root, 'tools', 'fixture');
      await mkdir(directory, { recursive: true });
      await writeFile(
        join(directory, 'tool.json'),
        JSON.stringify({
          name: 'fixture',
          inputSchema: { type: 'object' },
          execution: {
            command: process.execPath,
            args: ['-e', `process.stdout.write(${JSON.stringify(command)})`],
          },
        }),
      );
    }
    return new ToolPrecedenceFixture(globalDirectory, projectDirectory);
  }
}
