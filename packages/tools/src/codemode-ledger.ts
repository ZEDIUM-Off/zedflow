import { randomUUID } from 'node:crypto';
import type { JsonObject, NestedToolCallRecord, NestedToolCalls } from '@earendil-works/pi-ai';
import { sliceHead } from './text-slice.ts';

const MAX_LEDGER_CALLS = 100;
const MAX_ARGUMENT_BYTES = 1024;
const MAX_TOTAL_ARGUMENT_BYTES = 16 * 1024;

/** Owns call admission, identity and bounded metadata, never nested result bodies. */
export class CodemodeLedger {
  readonly #idPrefix: string;
  readonly #maxToolCalls: number;
  readonly #calls: NestedToolCallRecord[] = [];
  #callCount = 0;
  #argumentBytes = 0;
  #metadataComplete = true;

  public constructor(callId: string, maxToolCalls: number) {
    this.#idPrefix = `${sliceHead(callId, 128)}/${randomUUID()}`;
    this.#maxToolCalls = maxToolCalls;
  }

  public admit(): string {
    if (this.#callCount >= this.#maxToolCalls) {
      throw new Error('Code Mode nested tool call limit reached');
    }
    this.#callCount++;
    return `${this.#idPrefix}/${this.#callCount}`;
  }

  public record(raw: unknown, name: string, id: string): NestedToolCallRecord | undefined {
    if (this.#calls.length >= MAX_LEDGER_CALLS) {
      this.#metadataComplete = false;
      return undefined;
    }
    const record: NestedToolCallRecord = { id, name: sliceHead(name, 200), status: 'unfinished' };
    this.#arguments(record, raw);
    this.#calls.push(record);
    return record;
  }

  #arguments(record: NestedToolCallRecord, raw: unknown): void {
    const serialized = JSON.stringify(raw) ?? '';
    const bytes = Buffer.byteLength(serialized);
    if (
      raw !== null &&
      typeof raw === 'object' &&
      !Array.isArray(raw) &&
      bytes <= MAX_ARGUMENT_BYTES &&
      this.#argumentBytes + bytes <= MAX_TOTAL_ARGUMENT_BYTES
    ) {
      record.arguments = structuredClone(raw) as JsonObject;
      this.#argumentBytes += bytes;
    } else {
      record.argumentsBytes = bytes;
      this.#metadataComplete = false;
    }
  }

  public hasUnfinishedCalls(): boolean {
    return this.#calls.some((call) => call.status === 'unfinished');
  }

  public snapshot(): NestedToolCalls {
    return {
      calls: structuredClone(this.#calls),
      complete: this.#metadataComplete && !this.hasUnfinishedCalls(),
    };
  }
}
