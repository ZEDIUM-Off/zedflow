import { readFile, readdir } from 'node:fs/promises';
import { join } from 'node:path';
import type { TranscriptContext } from '@earendil-works/pi-ai';
import { vi } from 'vite-plus/test';
import { Command } from '../../../packages/cli/src/index.ts';
import { VirtualTerminal } from '../../../packages/tui/test/virtual-terminal.ts';
import { type CliFixture, isolatedAuth } from './fixture.ts';
import { scriptedProvider } from './scripted-provider.ts';

export class TerminalStartup {
  readonly #fixture: CliFixture;
  readonly #fake = scriptedProvider('direct');
  readonly #terminal = new VirtualTerminal();
  #error = '';
  #restore: (() => void) | undefined;
  readonly #options = {
    provider: this.#fake.provider,
    authContext: isolatedAuth,
    write: () => undefined,
    error: (text: string) => {
      this.#error += text;
    },
  };

  public constructor(fixture: CliFixture) {
    this.#fixture = fixture;
  }
  public get error(): string {
    return this.#error;
  }
  public get contexts(): TranscriptContext[] {
    return this.#fake.contexts;
  }

  public async login(): Promise<number> {
    return new Command({
      ...this.#options,
      interaction: { prompt: async () => 'synthetic-start-key', notify: () => undefined },
    }).run([...this.#fixture.flags, 'auth', 'login', '--api-key']);
  }

  public failStart(): void {
    const start = vi.spyOn(this.#terminal, 'start').mockImplementation(() => {
      throw new Error('terminal unavailable');
    });
    this.#restore = () => start.mockRestore();
  }

  public async run(prompt: string): Promise<number> {
    return new Command({ ...this.#options, terminal: this.#terminal }).run([
      ...this.#fixture.flags,
      'run',
      'agent',
      '--tui',
      '--prompt',
      prompt,
    ]);
  }

  public async records(): Promise<string> {
    const dir = join(this.#fixture.cwd, '.zedflow', 'runs');
    return (
      await Promise.all((await readdir(dir)).map((name) => readFile(join(dir, name), 'utf8')))
    ).join('\n');
  }

  public restore(): void {
    this.#restore?.();
  }
}
