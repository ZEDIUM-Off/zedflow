import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { MachineManager } from '../../../packages/manager/src/index.ts';
import { command } from './command.ts';
import { RecordingProcess } from './recording-process.ts';
import { RegistryFixture } from './registry-fixture.ts';

// Each test owns its HOME, sources, registry, runtime, stores and subprocess environment.
export class ManagerFixture {
  readonly #directory: string;
  readonly #processRunner: RecordingProcess;
  readonly #registry: RegistryFixture;
  readonly #manager: MachineManager;

  private constructor(
    directory: string,
    processRunner: RecordingProcess,
    registry: RegistryFixture,
  ) {
    this.#directory = directory;
    this.#processRunner = processRunner;
    this.#registry = registry;
    this.#manager = new MachineManager({
      directory: this.root,
      process: this.#processRunner,
      registry: registry.url(),
    });
  }

  public static async create(): Promise<ManagerFixture> {
    const base = resolve('.artifacts/manager-tests');
    await mkdir(base, { recursive: true });
    const directory = await mkdtemp(join(base, 'case-'));
    for (const name of ['home', 'tmp', 'config']) {
      await mkdir(join(directory, name));
    }
    const processRunner = new RecordingProcess();
    const registry = new RegistryFixture(join(directory, 'registry'));
    await registry.start();
    return new ManagerFixture(directory, processRunner, registry);
  }

  public get directory(): string {
    return this.#directory;
  }
  public get root(): string {
    return join(this.#directory, 'home', '.zedflow');
  }
  public get source(): string {
    return join(this.#directory, 'sources', 'agent');
  }
  public get processRunner(): RecordingProcess {
    return this.#processRunner;
  }
  public get registry(): RegistryFixture {
    return this.#registry;
  }
  public get manager(): MachineManager {
    return this.#manager;
  }

  public async importExport(entry: string, name: string): Promise<string> {
    return command({
      directory: this.#directory,
      command: process.execPath,
      args: [
        '--input-type=module',
        '-e',
        `console.log((await import(${JSON.stringify(pathToFileURL(entry).href)})).${name})`,
      ],
    });
  }

  public async close(): Promise<void> {
    await this.#registry.close();
    await rm(this.#directory, { recursive: true, force: true });
  }
}
