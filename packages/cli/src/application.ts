import { join, resolve } from 'node:path';
import type {
  AuthContext,
  Provider,
  AssistantMessageEvent,
  ToolResultMessage,
} from '@earendil-works/pi-ai';
import { Configuration } from '@zf/config';
import type { Settings } from '@zf/config';
import { OpenAIRuntime, TypeScriptModuleLoader } from '@zf/capabilities';
import { JsonCredentialStore, JsonlRunStore } from '@zf/storage';
import { RunCoordinator } from '@zf/run';
import { XStateEngine } from '@zf/xstate';
import { MachineManager } from '@zf/manager';
import { ResourceLoader } from './generation.ts';

export interface ApplicationOptions {
  readonly cwd: string;
  readonly globalDirectory: string;
  readonly recordsDirectory?: string;
  readonly overrides?: Readonly<Settings>;
  readonly provider?: Provider;
  readonly authContext?: AuthContext;
  readonly onModelEvent?: (event: AssistantMessageEvent) => void;
  readonly onToolUpdate?: (result: ToolResultMessage) => void;
}
/** CLI composition only. RunCoordinator remains the sole run lifecycle coordinator. */
export class Zedflow {
  public readonly runtime: OpenAIRuntime;
  public readonly configuration: Configuration;
  public readonly resources: ResourceLoader;
  public readonly runs: RunCoordinator;
  readonly #cwd: string;
  readonly #globalDirectory: string;
  public constructor(options: ApplicationOptions) {
    this.#cwd = resolve(options.cwd);
    this.#globalDirectory = resolve(options.globalDirectory);
    this.runtime = new OpenAIRuntime({
      credentials: new JsonCredentialStore(join(this.#globalDirectory, 'auth.json')),
      ...(options.provider ? { provider: options.provider } : {}),
      ...(options.authContext ? { authContext: options.authContext } : {}),
    });
    this.configuration = new Configuration({
      globalDirectory: this.#globalDirectory,
      projectDirectory: this.#cwd,
    });
    const engine = new XStateEngine();
    this.resources = new ResourceLoader({
      ...options,
      cwd: this.#cwd,
      globalDirectory: this.#globalDirectory,
      runtime: this.runtime,
      loader: new TypeScriptModuleLoader(engine.modules),
    });
    this.runs = new RunCoordinator({
      engine,
      loader: this.resources,
      store: new JsonlRunStore(options.recordsDirectory ?? join(this.#cwd, '.zedflow', 'runs')),
    });
  }
  public manager(scope: 'global' | 'project'): MachineManager {
    return new MachineManager({
      directory: scope === 'global' ? this.#globalDirectory : join(this.#cwd, '.zedflow'),
    });
  }
  public async launch(id: string, origin?: string): Promise<string> {
    return this.runs.launch(`machine:${id}`, origin);
  }
}
