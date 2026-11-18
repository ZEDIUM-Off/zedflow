import { resolve } from 'node:path';
import type { CodemodeToolOptions } from './codemode.ts';
const boundedInteger = (value: number | undefined, fallback: number, maximum: number): number => {
  const resolved = value ?? fallback;
  if (!Number.isInteger(resolved) || resolved <= 0 || resolved > maximum) {
    throw new Error(`Expected an integer between 1 and ${maximum}`);
  }
  return resolved;
};
/** Validates host ceilings once; source options can only lower them. */
export class CodemodeLimits {
  readonly #timeoutMs: number;
  readonly #memoryLimitBytes: number;
  readonly #maxOutputTokens: number;
  readonly #maxToolCalls: number;
  readonly #maxConcurrentExecutions: number;
  readonly #artifactDirectory: string;
  public constructor(options: CodemodeToolOptions) {
    this.#timeoutMs = boundedInteger(options.timeoutMs, 60000, 300000);
    this.#memoryLimitBytes = boundedInteger(
      options.memoryLimitBytes,
      64 * 1024 * 1024,
      256 * 1024 * 1024,
    );
    this.#maxOutputTokens = boundedInteger(options.maxOutputTokens, 10000, 100000);
    this.#maxToolCalls = boundedInteger(options.maxToolCalls, 100, 1000);
    this.#maxConcurrentExecutions = boundedInteger(options.maxConcurrentExecutions, 4, 16);
    this.#artifactDirectory = options.artifactDirectory ?? resolve('.artifacts/tools');
  }
  public timeoutMs(requested?: number): number {
    return Math.min(this.#timeoutMs, requested ?? this.#timeoutMs);
  }
  public get memoryLimitBytes(): number {
    return this.#memoryLimitBytes;
  }
  public maxChars(requested?: number): number {
    return Math.min(this.#maxOutputTokens, requested ?? this.#maxOutputTokens) * 4;
  }
  public get maxToolCalls(): number {
    return this.#maxToolCalls;
  }
  public get maxConcurrentExecutions(): number {
    return this.#maxConcurrentExecutions;
  }
  public get artifactDirectory(): string {
    return this.#artifactDirectory;
  }
}
