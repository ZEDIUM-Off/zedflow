import { createHash } from 'node:crypto';
import type { ExecutableTool, ToolDeclaration, Json } from '@zf/core';
import { json } from '@zf/core';
import { ToolCatalog, CommandTool } from '@zf/capabilities';

/** A preparation-local tool pool; its fingerprint includes the resolved command declarations. */
export class ToolPool {
  readonly #cwd: string;
  readonly #roots: readonly string[];
  readonly #tools = new Map<string, ExecutableTool>();
  #fingerprint = '';
  public constructor(cwd: string, roots: readonly string[]) {
    this.#cwd = cwd;
    this.#roots = roots;
  }
  public async load(): Promise<void> {
    const { createBuiltinTools } = await import('@zf/tools');
    const builtins = createBuiltinTools({ cwd: this.#cwd });
    const native = new Map(builtins.map((tool) => [tool.declaration.name, tool]));
    const catalog = new ToolCatalog({
      roots: this.#roots,
      builtins: builtins.map((tool) => ({
        name: tool.declaration.name,
        description: tool.declaration.description,
        inputSchema: json(tool.declaration.parameters),
        execution: { builtin: tool.declaration.name },
      })),
    });
    const declarations = await catalog.list();
    for (const [id, declaration] of declarations) {
      this.#tools.set(id, this.#resolve(id, declaration, native));
    }
    // Capture before factory bindings expose mutable tool schemas.
    this.#fingerprint = createHash('sha256')
      .update(JSON.stringify([...declarations]))
      .digest('hex');
  }
  public ids(): readonly string[] {
    return [...this.#tools.keys()];
  }
  public select(selection: Json | undefined): ExecutableTool[] {
    if (!Array.isArray(selection) || selection.some((name) => typeof name !== 'string')) {
      throw new Error('Invalid tool selection');
    }
    const selected = selection.map((name) => this.#tool(name));
    if (new Set(selected.map((tool) => tool.declaration.name)).size !== selected.length) {
      throw new Error('Selected tools have conflicting model-facing names');
    }
    return selected;
  }
  public fingerprint(): string {
    return this.#fingerprint;
  }
  #tool(name: Json): ExecutableTool {
    if (typeof name !== 'string') {
      throw new Error('Invalid tool ID');
    }
    const tool = this.#tools.get(name);
    if (!tool) {
      throw new Error(`Unknown tool: ${name}`);
    }
    return tool;
  }
  #resolve(
    id: string,
    declaration: ToolDeclaration,
    native: ReadonlyMap<string, ExecutableTool>,
  ): ExecutableTool {
    if (id === 'codemode') {
      throw new Error('codemode is reserved for the native Code Mode capability');
    }
    if (!('builtin' in declaration.execution)) {
      return new CommandTool({ id, declaration, cwd: this.#cwd });
    }
    const tool = native.get(declaration.execution.builtin);
    if (!tool) {
      throw new Error(`Unknown built-in tool: ${id}`);
    }
    return tool;
  }
}
