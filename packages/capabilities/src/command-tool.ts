import { isAbsolute } from 'node:path';
import { compile } from '@earendil-works/pi-schema';
import { validateToolCall } from '@earendil-works/pi-ai';
import type { Tool, ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool, ToolDeclaration, ToolExecution } from '@zf/core';
import { CommandRun } from './command-run.ts';

export interface CommandToolOptions {
  readonly id: string;
  readonly declaration: ToolDeclaration;
  readonly cwd: string;
}
const nativeToolName = (id: string): string => {
  const name = id.replaceAll('/', '__');
  if (!/^[a-zA-Z0-9_-]{1,64}$/.test(name)) {
    throw new Error(`Tool ID cannot form a native model name: ${id}`);
  }
  return name;
};
/** Declarative local commands: shell-free, JSON stdin, bounded textual native results. */
export class CommandTool implements ExecutableTool {
  public readonly declaration: Tool;
  readonly #options: CommandToolOptions;
  public constructor(options: CommandToolOptions) {
    this.#options = options;
    if (
      'command' in options.declaration.execution &&
      /[\\/]/.test(options.declaration.execution.command) &&
      !isAbsolute(options.declaration.execution.command)
    ) {
      throw new Error('Relative command paths must be resolved by the tool catalog');
    }
    const name = nativeToolName(options.id);
    const schema = options.declaration.inputSchema;
    if (!schema || typeof schema !== 'object' || Array.isArray(schema)) {
      throw new Error('Tool schema must be an object');
    }
    compile(schema);
    this.declaration = {
      name,
      description: options.declaration.description ?? options.id,
      parameters: schema,
    };
  }
  public async execute(input: ToolExecution): Promise<ToolResultMessage> {
    input.signal.throwIfAborted();
    const args: unknown = validateToolCall([this.declaration], input.call);
    const stdin = JSON.stringify(args);
    if (Buffer.byteLength(stdin) > 262144) {
      throw new Error('Command tool arguments exceed 256 KiB');
    }
    const execution = this.#options.declaration.execution;
    if (!('command' in execution)) {
      throw new Error('Expected a command tool');
    }
    return new CommandRun(execution, this.#options.cwd, input).result(stdin);
  }
}
