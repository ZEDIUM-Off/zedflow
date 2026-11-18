import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { Configuration } from '../../../packages/config/src/index.ts';

export class ConfigurationFixture {
  readonly #directory: string;
  readonly #globalDirectory: string;
  readonly #projectDirectory: string;
  readonly #configuration: Configuration;

  private constructor(directory: string) {
    this.#directory = directory;
    this.#globalDirectory = join(directory, 'home', '.zedflow');
    this.#projectDirectory = join(directory, 'project');
    this.#configuration = new Configuration({
      globalDirectory: this.#globalDirectory,
      projectDirectory: this.#projectDirectory,
    });
  }

  public static async create(): Promise<ConfigurationFixture> {
    const fixtures = resolve('.artifacts/config-tests');
    await mkdir(fixtures, { recursive: true });
    return new ConfigurationFixture(await mkdtemp(join(fixtures, 'case-')));
  }

  public get directory(): string {
    return this.#directory;
  }
  public get globalDirectory(): string {
    return this.#globalDirectory;
  }
  public get projectDirectory(): string {
    return this.#projectDirectory;
  }
  public get configuration(): Configuration {
    return this.#configuration;
  }

  public async seedLayers(): Promise<void> {
    const configuration = this.#configuration;
    await configuration.set('global', 'custom', {
      a: 'global',
      b: 'global',
      c: 'global',
      d: 'global',
      e: 'global',
    });
    await configuration.set('global', 'machines.domain/agent', {
      custom: {
        b: 'global-machine',
        c: 'global-machine',
        d: 'global-machine',
        e: 'global-machine',
      },
      tools: ['global-machine-tool'],
    });
    await configuration.set('project', 'custom', { c: 'project', d: 'project', e: 'project' });
    await configuration.set('project', 'machines.domain/agent', {
      custom: { d: 'project-machine', e: 'project-machine' },
      tools: ['project-machine-tool'],
    });
    await configuration.set('project', 'machines.other', { custom: { a: 'wrong-machine' } });
  }

  public async dispose(): Promise<void> {
    await rm(this.#directory, { recursive: true, force: true });
  }
}
