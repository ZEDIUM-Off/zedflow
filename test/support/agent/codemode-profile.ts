import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import type { ExecutableTool, Execution } from '@zf/core';
import { createBuiltinTools, createCodemodeTool } from '../../../packages/tools/src/index.ts';
import { createMachine } from '../../../packages/machines/agent/machine.ts';
import { startMachine } from './execution.ts';
import { png, call } from './messages.ts';
import { answer, calls } from './responses.ts';
import { NativeOpenAI } from './native-openai.ts';

const code = `
    if (ALL_TOOLS.some(tool => tool.name === 'grep')) throw new Error('unselected tool exposed');
    await tools.write({path:'vm.txt', content:'before'});
    await tools.edit({path:'vm.txt', oldText:'before', newText:'after'});
    const value = await tools.read({path:'vm.txt'});
    const shell = await tools.bash({command:'printf vm-shell; exit 3'});
    image(await tools.read({path:'picture.png'}));
    text({value, exitCode:shell.exit_code, output:shell.output});
    return value;
  `;

export class CodemodeProfile {
  readonly #native: NativeOpenAI;
  readonly #tools: readonly ExecutableTool[];
  readonly #execution: Execution;
  readonly #updates: ToolResultMessage[] = [];

  private constructor(
    cwd: string,
    mode: 'on' | 'only',
    native: NativeOpenAI,
    tools: readonly ExecutableTool[],
  ) {
    this.#native = native;
    this.#tools = tools;
    this.#execution = startMachine(
      createMachine(
        native.bindings(cwd, {
          tools,
          config: { codemode: { mode } },
          onToolUpdate: (update) => this.#updates.push(update),
        }),
      ),
    ).execution;
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
  public get updates(): ToolResultMessage[] {
    return this.#updates;
  }

  public static async create(cwd: string, mode: 'on' | 'only'): Promise<CodemodeProfile> {
    await writeFile(join(cwd, 'picture.png'), Buffer.from(png, 'base64'));
    const selected = createBuiltinTools({ cwd }).filter((tool) =>
      ['read', 'write', 'edit', 'bash'].includes(tool.declaration.name),
    );
    const codemode = createCodemodeTool({
      tools: selected,
      artifactDirectory: join(cwd, 'outputs'),
    });
    const tools = mode === 'only' ? [codemode] : [...selected, codemode];
    const native = await NativeOpenAI.create([
      calls(call('codemode', { code }, 'script-1')),
      answer('VM finished'),
    ]);
    return new CodemodeProfile(cwd, mode, native, tools);
  }
}
