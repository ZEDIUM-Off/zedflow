import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { AssistantMessageEvent, ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool, Execution, Frame } from '@zf/core';
import { createBuiltinTools } from '../../../packages/tools/src/index.ts';
import { createMachine } from '../../../packages/machines/agent/machine.ts';
import { startMachine } from './execution.ts';
import { png, call } from './messages.ts';
import { answer, calls } from './responses.ts';
import { NativeOpenAI } from './native-openai.ts';

export class BuiltinConversation {
  readonly #native: NativeOpenAI;
  readonly #tools: readonly ExecutableTool[];
  readonly #execution: Execution;
  readonly #frames: Frame[];
  readonly #modelEvents: AssistantMessageEvent[] = [];
  readonly #updates: ToolResultMessage[] = [];

  private constructor(cwd: string, native: NativeOpenAI, tools: readonly ExecutableTool[]) {
    this.#native = native;
    this.#tools = tools;
    const { execution, frames } = startMachine(
      createMachine(
        native.bindings(cwd, {
          tools,
          onModelEvent: (event) => this.#modelEvents.push(event),
          onToolUpdate: (update) => this.#updates.push(update),
        }),
      ),
    );
    this.#execution = execution;
    this.#frames = frames;
  }

  public get native(): NativeOpenAI {
    return this.#native;
  }
  public get tools(): readonly ExecutableTool[] {
    return this.#tools;
  }
  public get execution(): Execution {
    return this.#execution;
  }
  public get frames(): Frame[] {
    return this.#frames;
  }
  public get modelEvents(): AssistantMessageEvent[] {
    return this.#modelEvents;
  }
  public get updates(): ToolResultMessage[] {
    return this.#updates;
  }

  public static async create(cwd: string): Promise<BuiltinConversation> {
    await writeFile(join(cwd, 'picture.dat'), Buffer.from(png, 'base64'));
    const native = await NativeOpenAI.create([
      calls(
        call('write', { path: 'file.txt', content: 'written by agent' }, 'write-1'),
        call('read', { path: 'file.txt' }, 'read-1'),
        call('read', { path: 'picture.dat' }, 'image-1'),
        call('bash', { command: 'printf offline-progress' }, 'bash-1'),
      ),
      answer(),
      answer('second turn'),
    ]);
    const tools = createBuiltinTools({ cwd });
    return new BuiltinConversation(cwd, native, tools);
  }
}
