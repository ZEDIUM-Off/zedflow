import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { TranscriptContext } from '@earendil-works/pi-ai';
import { Command } from '../../../packages/cli/src/index.ts';
import { VirtualTerminal } from '../../../packages/tui/test/virtual-terminal.ts';
import { type CliFixture, isolatedAuth } from './fixture.ts';
import { scriptedProvider } from './scripted-provider.ts';

export class TerminalSession {
  readonly #fixture: CliFixture;
  #finish: (() => void) | undefined;
  readonly #fake = scriptedProvider('only', {
    hold: (finish) => {
      this.#finish = finish;
    },
  });
  readonly #terminal = new VirtualTerminal(100, 32);
  #pending: Promise<number> | undefined;

  public constructor(fixture: CliFixture) {
    this.#fixture = fixture;
  }
  public get terminal(): VirtualTerminal {
    return this.#terminal;
  }
  public get contexts(): TranscriptContext[] {
    return this.#fake.contexts;
  }

  public async login(): Promise<number> {
    return new Command({
      ...this.#options,
      interaction: { prompt: async () => 'synthetic-ui-key', notify: () => undefined },
    }).run([...this.#fixture.flags, 'auth', 'login', '--api-key']);
  }

  public async start(): Promise<void> {
    await mkdir(join(this.#fixture.cwd, '.zedflow'), { recursive: true });
    await this.configure(['write'], 'only');
    const command = new Command({ ...this.#options, terminal: this.#terminal });
    this.#pending = command.run([...this.#fixture.flags, 'run', 'agent', '--tui']);
  }

  public async configure(tools: string[], mode: 'only' | 'on'): Promise<void> {
    await writeFile(
      join(this.#fixture.cwd, '.zedflow', 'settings.json'),
      JSON.stringify({ tools, codemode: { mode } }),
    );
  }

  public submit(value: string): void {
    this.#terminal.sendInput(value);
    this.#terminal.sendInput('\r');
  }

  public async screen(): Promise<string> {
    return (await this.#terminal.screen()).join('\n');
  }
  public finish(): void {
    this.#finish?.();
  }

  public result(): Promise<number> {
    if (!this.#pending) {
      throw new Error('Terminal session has not started');
    }
    return this.#pending;
  }

  public async close(): Promise<void> {
    this.#finish?.();
    if (this.#terminal.running) {
      this.submit('/quit');
    }
    await this.#pending;
  }

  readonly #options = {
    provider: this.#fake.provider,
    authContext: isolatedAuth,
    write: () => undefined,
    error: () => undefined,
  };
}
