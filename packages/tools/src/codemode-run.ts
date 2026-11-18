// Adapted from Pi 1.5.2 coding-agent/src/extensions/codemode/{tool,execute}.ts (MIT).
// Copyright Mario Zechner and Earendil Works; see ../LICENSE and ../PROVENANCE.json.
import { parseCodemodeSource } from '@earendil-works/pi-codemode';
import { validateToolArguments } from '@earendil-works/pi-ai';
import type { Tool, ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool, ToolExecution } from '@zf/core';
import { errorText, resultMessage } from './definition.ts';
import type { DefinitionResult } from './definition.ts';
import { CodemodeExecution } from './codemode-execution.ts';
import type { CodemodeLimits } from './codemode-limits.ts';
import type { CodemodeAdmission } from './codemode-admission.ts';
import { CodemodeWorker } from './codemode-worker.ts';
import { CodemodeOutput } from './codemode-output.ts';

/** Owns one host execution from validation through final metadata and worker cleanup. */
export class CodemodeRun {
  readonly #input: ToolExecution;
  readonly #limits: CodemodeLimits;
  readonly #admission: CodemodeAdmission;
  readonly #startedAt = performance.now();
  readonly #execution: CodemodeExecution;
  #worker: CodemodeWorker | undefined;

  public constructor(input: ToolExecution, limits: CodemodeLimits, admission: CodemodeAdmission) {
    this.#input = input;
    this.#limits = limits;
    this.#admission = admission;
    this.#execution = new CodemodeExecution(input, limits.maxToolCalls);
  }

  public async execute(
    declaration: Tool,
    tools: readonly ExecutableTool[],
  ): Promise<ToolResultMessage> {
    try {
      this.#input.signal.throwIfAborted();
      if (this.#input.call.name !== 'codemode') {
        throw new Error('Expected codemode call');
      }
      const args: unknown = validateToolArguments(declaration, this.#input.call);
      const { code: source } = args as { code: string };
      return await this.#run(source, tools);
    } catch (error) {
      this.#settle();
      return this.#message({ content: [{ type: 'text', text: errorText(error) }], isError: true });
    } finally {
      this.#settle();
      await this.#worker?.close();
    }
  }

  async #run(source: string, tools: readonly ExecutableTool[]): Promise<ToolResultMessage> {
    const parsed = parseCodemodeSource(source);
    this.#worker = new CodemodeWorker(this.#input.signal, this.#admission);
    const sandbox = this.#worker.open(
      this.#execution.tools(tools),
      this.#limits,
      parsed.options.timeoutMs,
    );
    this.#execution.start();
    const result = await sandbox.execute(parsed.code, { signal: this.#input.signal });
    this.#settle();
    const output = new CodemodeOutput(result, this.#limits.artifactDirectory);
    output.describe(result, this.#execution);
    return this.#message(
      await output.result(result.ok, this.#limits.maxChars(parsed.options.maxOutputTokens)),
    );
  }

  #settle(): void {
    this.#worker?.settle();
    this.#execution.finalize();
  }

  #message(result: DefinitionResult): ToolResultMessage {
    return {
      ...resultMessage(this.#input, result, performance.now() - this.#startedAt),
      nestedCalls: this.#execution.snapshot(),
    };
  }
}
