import type { Api, Model } from '@earendil-works/pi-ai';
import { ModuleCatalog, TypeScriptModuleLoader } from '@zf/capabilities';
import type { Execution } from '@zf/core';
import { XStateEngine } from '@zf/xstate';
import { BUILTIN_MACHINES_DIRECTORY } from '../../../packages/machines/src/index.ts';
import { createMachine } from '../../../packages/machines/agent/machine.ts';
import { startMachine } from './execution.ts';
import { NativeOpenAI } from './native-openai.ts';
import { answer } from './responses.ts';

export class AgentManifest {
  readonly #cwd: string;
  readonly #native: NativeOpenAI;
  readonly #catalog: ModuleCatalog;
  #second: NativeOpenAI | undefined;

  private constructor(cwd: string, native: NativeOpenAI) {
    this.#cwd = cwd;
    this.#native = native;
    const engine = new XStateEngine();
    this.#catalog = new ModuleCatalog({
      roots: [{ directory: BUILTIN_MACHINES_DIRECTORY, scope: 'builtin' }],
      loader: new TypeScriptModuleLoader(engine.modules),
      bindings: async () => native.bindings(cwd),
    });
  }

  public static async create(cwd: string): Promise<AgentManifest> {
    return new AgentManifest(cwd, await NativeOpenAI.create([answer('ordinary loader')]));
  }
  public get catalog(): ModuleCatalog {
    return this.#catalog;
  }
  public async load(): Promise<Execution> {
    const source = await this.#catalog.load(await this.#catalog.resolve('agent'));
    return startMachine(source.module).execution;
  }
  public alternate(): Model<Api> {
    const selected = this.#native.models.getModel('openai', 'gpt-6.1-terra');
    if (!selected) {
      throw new Error('Pinned alternate native model is missing');
    }
    return selected;
  }
  public get second(): NativeOpenAI {
    if (!this.#second) {
      throw new Error('Configured model has not been started');
    }
    return this.#second;
  }
  public async configured(selected: Model<Api>): Promise<Execution> {
    this.#second = await NativeOpenAI.create([answer('configured model'), answer('bound model')]);
    return startMachine(
      createMachine(
        this.#second.bindings(this.#cwd, {
          config: {
            model: { provider: 'openai', id: selected.id },
            systemPrompt: 'Fixture instructions',
          },
        }),
      ),
    ).execution;
  }
  public bound(selected: Model<Api>): Execution {
    const second = this.second;
    return startMachine(
      createMachine(
        second.bindings(this.#cwd, {
          model: second.model,
          config: { model: { provider: 'openai', id: selected.id } },
        }),
      ),
    ).execution;
  }
}
