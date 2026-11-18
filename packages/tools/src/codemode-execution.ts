// Adapted from Pi 1.5.2 coding-agent/src/extensions/codemode/execute.ts (MIT).
// Copyright Mario Zechner and Earendil Works; see ../LICENSE and ../PROVENANCE.json.
import type { NestedToolCallRecord, NestedToolCalls } from '@earendil-works/pi-ai';
import type { CodemodeJsonSchema, CodemodeTool } from '@earendil-works/pi-codemode';
import type { ExecutableTool, ToolExecution } from '@zf/core';
import { resultMessage } from './definition.ts';
import { CodemodeLedger } from './codemode-ledger.ts';
import { CodemodeToolCall } from './codemode-tool-call.ts';

/** Owns the nested-call update window and outstanding effects of one script execution. */
export class CodemodeExecution {
  readonly #input: ToolExecution;
  readonly #ledger: CodemodeLedger;
  readonly #outstandingCalls = new Set<CodemodeToolCall>();
  #finalized = false;

  public constructor(input: ToolExecution, maxToolCalls: number) {
    this.#input = input;
    this.#ledger = new CodemodeLedger(input.call.id, maxToolCalls);
  }

  public tools(tools: readonly ExecutableTool[]): CodemodeTool[] {
    return tools.map((tool) => ({
      name: tool.declaration.name,
      description: tool.declaration.description,
      inputSchema: tool.declaration.parameters as CodemodeJsonSchema,
      execute: (raw, context) =>
        new CodemodeToolCall(tool, context, this, this.#ledger).execute(raw),
    }));
  }

  public start(): void {
    this.publish();
  }

  public finalize(): void {
    this.#finalized = true;
  }

  public hasUnfinishedCalls(): boolean {
    return this.#outstandingCalls.size > 0 || this.#ledger.hasUnfinishedCalls();
  }

  public snapshot(): NestedToolCalls {
    return this.#ledger.snapshot();
  }

  public live(signal: AbortSignal): boolean {
    return !this.#finalized && !this.#input.signal.aborted && !signal.aborted;
  }

  public assertLive(signal: AbortSignal): void {
    if (!this.live(signal)) {
      throw new Error(
        'Nested tool outcome is unfinished/uncertain after execution cancellation or finalization',
      );
    }
  }

  public publish(record?: NestedToolCallRecord): void {
    if (this.#finalized || this.#input.signal.aborted) {
      return;
    }
    // Bounded single-call deltas, not cumulative snapshots or nested result bodies.
    this.#input.onUpdate?.({
      ...resultMessage(this.#input, { content: [] }),
      nestedCalls: { calls: record ? [structuredClone(record)] : [], complete: false },
    });
  }

  public begin(call: CodemodeToolCall): void {
    this.#outstandingCalls.add(call);
  }

  public finish(call: CodemodeToolCall): void {
    this.#outstandingCalls.delete(call);
  }
}
