import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { Zedflow } from '../../../packages/cli/src/index.ts';
import { type CliFixture, isolatedAuth } from './fixture.ts';
import { DeferredReloadProvider } from './deferred-reload-provider.ts';

const skill = (description: string): string =>
  `---\nname: review\ndescription: ${description}\n---\nLazy skill body\n`;

export class ReloadFixture {
  readonly #skillDirectory: string;
  readonly #native = new DeferredReloadProvider();
  readonly #app: Zedflow;

  public constructor(f: CliFixture) {
    this.#skillDirectory = join(f.cwd, '.zedflow', 'skills', 'review');
    this.#app = new Zedflow({
      cwd: f.cwd,
      globalDirectory: f.globalDirectory,
      provider: this.#native.provider,
      authContext: isolatedAuth,
    });
  }

  public get app(): Zedflow {
    return this.#app;
  }
  public get native(): DeferredReloadProvider {
    return this.#native;
  }

  public async start(): Promise<string> {
    await mkdir(this.#skillDirectory, { recursive: true });
    await writeFile(join(this.#skillDirectory, 'SKILL.md'), skill('before reload'));
    await this.#app.runtime.models.login('openai', 'api_key', {
      prompt: async () => 'synthetic-reload-key',
      notify: () => undefined,
    });
    const id = await this.#app.launch('agent');
    this.#app.runs.send(id, { type: 'PROMPT', text: 'first' });
    await this.#native.entered;
    return id;
  }

  public async prepareCandidate(): Promise<void> {
    await this.#app.configuration.set('project', 'tools', []);
    await this.#app.configuration.set('project', 'codemode.mode', 'only');
    await writeFile(join(this.#skillDirectory, 'SKILL.md'), skill('after reload'));
  }
}
