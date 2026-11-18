import { ModelsError } from '@earendil-works/pi-ai';
import type { OpenAIRuntime } from '@zf/capabilities';
import type { CommandOptions } from '../command-options.ts';
import type { CommandLine } from '../command-line.ts';
import type { CommandOutput } from '../command-output.ts';
import type { CommandLifecycle } from '../command-lifecycle.ts';
import { DeviceIdentity } from '../device-identity.ts';
import { TerminalAuthInteraction } from '../terminal-auth-interaction.ts';
export class AuthCommand {
  readonly #runtime: OpenAIRuntime;
  readonly #output: CommandOutput;
  readonly #options: CommandOptions;
  public constructor(runtime: OpenAIRuntime, output: CommandOutput, options: CommandOptions) {
    this.#runtime = runtime;
    this.#output = output;
    this.#options = options;
  }
  public async run(line: CommandLine, lifecycle: CommandLifecycle): Promise<number> {
    this.#validate(line);
    try {
      await this.#perform(line, lifecycle);
    } catch (error) {
      throw this.#failure(error, lifecycle.signal);
    }
    return 0;
  }
  #validate(line: CommandLine): void {
    if (line.args[1] !== undefined && line.args[1] !== 'openai') {
      throw new Error('Only OpenAI authentication is supported');
    }
    if (!['login', 'status', 'logout'].includes(line.args[0] ?? '') || line.args.length > 2) {
      throw new Error('Use auth login, status or logout');
    }
    if (line.args[0] === 'login') {
      this.#requireTerminal();
    }
  }
  #requireTerminal(): void {
    if (!this.#options.interaction && (!process.stdin.isTTY || !process.stderr.isTTY)) {
      throw new Error('Authentication login requires an interactive terminal');
    }
  }
  async #perform(line: CommandLine, lifecycle: CommandLifecycle): Promise<void> {
    if (line.args[0] === 'status') {
      this.#output.print(
        (await this.#runtime.models.checkAuth('openai', { signal: lifecycle.signal })) ?? {
          authenticated: false,
        },
      );
    } else if (line.args[0] === 'logout') {
      await this.#runtime.models.logout('openai', { signal: lifecycle.signal });
      this.#output.write(
        'Stored OpenAI credentials removed; ambient environment credentials are unchanged.\n',
      );
    } else if (line.args[0] === 'login') {
      await this.#login(line, lifecycle);
    } else {
      throw new Error('Use auth login, status or logout');
    }
  }
  async #login(line: CommandLine, lifecycle: CommandLifecycle): Promise<void> {
    this.#requireTerminal();
    const interaction =
      this.#options.interaction ?? new TerminalAuthInteraction(lifecycle.abort, lifecycle.signal);
    const identity = new DeviceIdentity(line.globalDirectory);
    await this.#runtime.models.login(
      'openai',
      line.values['api-key'] ? 'api_key' : 'oauth',
      {
        signal: lifecycle.signal,
        prompt: (prompt) => interaction.prompt(prompt),
        notify: (event) => interaction.notify(event),
      },
      { agentName: 'Zedflow', getDeviceId: () => identity.get() },
    );
    this.#output.write('OpenAI credentials saved.\n');
  }
  #failure(error: unknown, signal: AbortSignal): Error {
    if (signal.aborted) {
      return new Error('Authentication cancelled');
    }
    if (error instanceof ModelsError && error.code === 'oauth') {
      return new Error('OpenAI subscription authorization failed. Sign in again.');
    }
    if (error instanceof ModelsError && error.code === 'auth') {
      return new Error(
        'OpenAI credential storage or key resolution failed. Check permissions, settings and lock contention.',
      );
    }
    return new Error(
      'OpenAI authentication failed. Check the authentication method, network and credential storage; no credential values were logged.',
    );
  }
}
