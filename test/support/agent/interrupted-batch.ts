import type { Execution } from '@zf/core';
import { createMachine } from '../../../packages/machines/agent/machine.ts';
import { createWriteTool } from '../../../packages/tools/src/index.ts';
import { AbortResolvingTool } from './abort-resolving-tool.ts';
import { startMachine } from './execution.ts';
import { call } from './messages.ts';
import { NativeOpenAI } from './native-openai.ts';
import { answer, calls } from './responses.ts';

export class InterruptedBatch {
  readonly #wait = new AbortResolvingTool();
  readonly #native: NativeOpenAI;
  readonly #execution: Execution;

  private constructor(cwd: string, native: NativeOpenAI) {
    this.#native = native;
    this.#execution = startMachine(
      createMachine(native.bindings(cwd, { tools: [createWriteTool(cwd), this.#wait] })),
    ).execution;
  }

  public static async create(cwd: string): Promise<InterruptedBatch> {
    const native = await NativeOpenAI.create([
      answer('completed seed turn'),
      calls(
        call('write', { path: 'completed', content: 'kept' }, 'completed-call'),
        call('custom', { value: 'wait' }, 'active-call'),
        call('write', { path: 'skipped', content: 'must not run' }, 'skipped-call'),
      ),
      answer('continued after cancellation'),
    ]);
    return new InterruptedBatch(cwd, native);
  }

  public get wait(): AbortResolvingTool {
    return this.#wait;
  }
  public get native(): NativeOpenAI {
    return this.#native;
  }
  public get execution(): Execution {
    return this.#execution;
  }
}
