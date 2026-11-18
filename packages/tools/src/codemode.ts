// Adapted from Pi 1.5.2 coding-agent/src/extensions/codemode/{tool,execute}.ts (MIT).
// Copyright Mario Zechner and Earendil Works; see ../LICENSE and ../PROVENANCE.json.
import { renderToolSample } from '@earendil-works/pi-codemode';
import type { CodemodeJsonSchema } from '@earendil-works/pi-codemode';
import type { Tool, ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool, ToolExecution } from '@zf/core';
import { CodemodeLimits } from './codemode-limits.ts';
import { CodemodeAdmission } from './codemode-admission.ts';
import { CodemodeRun } from './codemode-run.ts';
export interface CodemodeToolOptions {
  readonly tools: readonly ExecutableTool[];
  readonly artifactDirectory?: string | undefined;
  /** Upper bounds; a script's @options may lower, never raise them. */
  readonly timeoutMs?: number | undefined;
  readonly memoryLimitBytes?: number | undefined;
  readonly maxOutputTokens?: number | undefined;
  readonly maxToolCalls?: number | undefined;
  readonly maxConcurrentExecutions?: number | undefined;
}
class CodemodeHost implements ExecutableTool {
  public readonly declaration: Tool;
  readonly #tools: readonly ExecutableTool[];
  readonly #limits: CodemodeLimits;
  readonly #admission: CodemodeAdmission;
  public constructor(options: CodemodeToolOptions) {
    this.#tools = options.tools.filter((tool) => tool.declaration.name !== 'codemode');
    const names = this.#tools.map((tool) => tool.declaration.name);
    if (new Set(names).size !== names.length) {
      throw new Error('Duplicate Code Mode tool names');
    }
    this.#limits = new CodemodeLimits(options);
    this.#admission = new CodemodeAdmission(this.#limits.maxConcurrentExecutions);
    this.declaration = {
      name: 'codemode',
      description: `Run raw JavaScript as an async function body in a QuickJS sandbox. Use await tools.<name>(args), text(value), image(imageBlock), console.log(), return, or exit(). No Node, filesystem, network or timers except selected tools. image() also saves emitted images to the artifact directory and reports their paths. Calls still running when the script ends receive cancellation requests; their effects are reported unfinished/uncertain, not undone. Tool errors reject, except built-in structured results (check exit_code). Built-in read resolves to text or an image block; bash/powershell resolve to {output,truncated,full_output_path?,exit_code,wall_time_seconds}. Other tools resolve to text unless details.structuredContent is present; multimodal custom tools resolve to {content,details}. store()/load() are local to this execution, not persisted. Optional first line: // @options: {"timeout_ms":1000,"max_output_tokens":1000}; options cannot exceed host limits. Nested tool mutations are not undone on failure.\n\nNested tools:\n${this.#tools.map((tool) => renderToolSample({ name: tool.declaration.name, description: tool.declaration.description, inputSchema: tool.declaration.parameters as CodemodeJsonSchema })).join('\n')}`,
      parameters: {
        type: 'object',
        properties: { code: { type: 'string' } },
        required: ['code'],
        additionalProperties: false,
      },
    };
  }
  public async execute(input: ToolExecution): Promise<ToolResultMessage> {
    return new CodemodeRun(input, this.#limits, this.#admission).execute(
      this.declaration,
      this.#tools,
    );
  }
}
export const createCodemodeTool = (options: CodemodeToolOptions): ExecutableTool =>
  new CodemodeHost(options);
