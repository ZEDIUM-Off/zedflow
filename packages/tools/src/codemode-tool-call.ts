// Adapted from Pi 1.5.2 coding-agent/src/extensions/codemode/execute.ts (MIT).
// Copyright Mario Zechner and Earendil Works; see ../LICENSE and ../PROVENANCE.json.
import { validateToolArguments } from '@earendil-works/pi-ai';
import type {
  JsonObject,
  NestedToolCallRecord,
  ToolCall,
  ToolResultMessage,
} from '@earendil-works/pi-ai';
import type { CodemodeToolContext } from '@earendil-works/pi-codemode';
import type { ExecutableTool } from '@zf/core';
import type { CodemodeExecution } from './codemode-execution.ts';
import type { CodemodeLedger } from './codemode-ledger.ts';
import { errorText } from './definition.ts';
import { resultError, scriptValue } from './script-value.ts';
import { sliceHead } from './text-slice.ts';

const prepareCall = (tool: ExecutableTool, raw: unknown, id: string): ToolCall => {
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) {
    throw new Error('Tool arguments must be an object');
  }
  const call = {
    type: 'toolCall' as const,
    id,
    name: tool.declaration.name,
    arguments: raw as JsonObject,
  };
  const copied = structuredClone(call.arguments);
  const prepared = tool.prepareArguments ? tool.prepareArguments(copied) : copied;
  const validated: unknown = validateToolArguments(tool.declaration, {
    ...call,
    arguments: prepared as JsonObject,
  });
  return { ...call, arguments: validated as JsonObject };
};

/** Owns one call's timing, single progress notification and observed ledger outcome. */
export class CodemodeToolCall {
  readonly #tool: ExecutableTool;
  readonly #context: CodemodeToolContext;
  readonly #execution: CodemodeExecution;
  readonly #ledger: CodemodeLedger;
  #record: NestedToolCallRecord | undefined;
  #startedAt = 0;
  #reportedProgress = false;

  public constructor(
    tool: ExecutableTool,
    context: CodemodeToolContext,
    execution: CodemodeExecution,
    ledger: CodemodeLedger,
  ) {
    this.#tool = tool;
    this.#context = context;
    this.#execution = execution;
    this.#ledger = ledger;
  }

  public async execute(raw: unknown): Promise<unknown> {
    const id = this.#begin(raw);
    try {
      const call = prepareCall(this.#tool, raw, id);
      this.#execution.assertLive(this.#context.signal);
      const result = await this.#tool.execute({
        call,
        signal: this.#context.signal,
        argumentsPrepared: true,
        onUpdate: () => this.#progress(),
      });
      return this.#observe(result);
    } catch (error) {
      // Abort-ignoring effects cannot be asserted complete after the observation window closes.
      if (this.#execution.live(this.#context.signal)) {
        this.#complete({ status: 'error', error: errorText(error) });
      }
      throw error;
    } finally {
      this.#execution.finish(this);
    }
  }

  #observe(result: ToolResultMessage): unknown {
    this.#execution.assertLive(this.#context.signal);
    const value = scriptValue(result);
    this.#complete(
      result.isError ? { status: 'error', error: resultError(result) } : { status: 'ok' },
    );
    return value;
  }

  #begin(raw: unknown): string {
    this.#execution.assertLive(this.#context.signal);
    const id = this.#ledger.admit();
    this.#record = this.#ledger.record(raw, this.#tool.declaration.name, id);
    this.#startedAt = performance.now();
    if (this.#record) {
      this.#execution.publish(this.#record);
    }
    this.#execution.begin(this);
    return id;
  }

  #progress(): void {
    if (!this.#reportedProgress && this.#record && this.#execution.live(this.#context.signal)) {
      this.#reportedProgress = true;
      this.#record.durationMs = performance.now() - this.#startedAt;
      this.#execution.publish(this.#record);
    }
  }

  #complete(outcome: { status: 'ok' | 'error'; error?: string }): void {
    if (!this.#record) {
      return;
    }
    this.#record.status = outcome.status;
    this.#record.durationMs = performance.now() - this.#startedAt;
    if (outcome.error !== undefined) {
      this.#record.error = sliceHead(outcome.error, 500);
    }
    this.#execution.publish(this.#record);
  }
}
