import { mkdir, mkdtemp } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import type { Execution, Frame, Json } from '@zf/core';
import { XStateEngine } from '@zf/xstate';
import { afterEach, beforeEach, expect, vi } from 'vite-plus/test';
import type { AgentContext } from '../../../packages/machines/agent/machine.ts';

const executions: AgentExecution[] = [];
beforeEach(() => {
  vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('Network forbidden in agent tests'));
});
afterEach(() => {
  executions.splice(0).forEach((execution) => execution.stop());
  expect(globalThis.fetch).not.toHaveBeenCalled();
  vi.restoreAllMocks();
});

export class AgentExecution {
  readonly #execution: Execution;
  readonly #frames: Frame[] = [];

  public constructor(machine: unknown, checkpoint?: Json) {
    const engine = new XStateEngine();
    this.#execution = engine.prepare(
      engine.identify(machine),
      (frame) => this.#frames.push(frame),
      checkpoint,
    );
    executions.push(this);
    this.#execution.start();
  }
  public get execution(): Execution {
    return this.#execution;
  }
  public get frames(): Frame[] {
    return this.#frames;
  }
  public stop(): void {
    this.#execution.stop();
  }
}

export const fixture = async (): Promise<string> => {
  const root = resolve('.artifacts/agent-tests');
  await mkdir(root, { recursive: true });
  return mkdtemp(join(root, 'case-'));
};
export const startMachine = (machine: unknown, checkpoint?: Json): AgentExecution =>
  new AgentExecution(machine, checkpoint);
export const context = (execution: Execution): AgentContext =>
  (execution.view() as unknown as { context: AgentContext }).context;
export const ready = (execution: Execution, modelCalls: number): Promise<void> =>
  vi.waitFor(
    () => {
      expect(execution.view()).toMatchObject({
        value: 'ready',
        context: { modelCalls, failure: null },
      });
    },
    { timeout: 10000 },
  );
