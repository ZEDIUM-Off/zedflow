import { Zedflow } from './application.ts';
import type { CommandOptions } from './command-options.ts';
import { CommandLine } from './command-line.ts';
import { CommandOutput } from './command-output.ts';
import { CommandLifecycle } from './command-lifecycle.ts';
import { ModelOutput } from './model-output.ts';
import { InstallationCommand, isInstallationCommand } from './commands/installation.ts';
import { AuthCommand } from './commands/auth.ts';
import { ConfigCommand } from './commands/config.ts';
import { RunCommand } from './commands/run.ts';
export type { CommandOptions } from './command-options.ts';
export const HELP = `zf — native XState machine kit
Usage: zf <command> [options]
  machines                         List discovered machines without executing them
  models                           List native OpenAI models
  run [agent]                      Interactive agent when a terminal is available
  run [agent] --prompt TEXT         Run one turn without a TUI (or use --tui)
  run <id> --event JSON             Send one event and print the current state
  config show [id]                  Effective settings and provenance (default agent)
  config set <path> <JSON>          Set a setting; --machine scopes it to one machine
  config unset <path>              Remove a setting
  auth login [openai] [--api-key]   Interactive API-key or subscription OAuth login
  auth status | auth logout        Native auth metadata / remove stored credentials
  install <source>                  Install local:, git: or npm: machines
  update <id> | remove <id>         Change an owned installation
  installed | restore --frozen     List receipts / restore exact locked resolutions
Options: --cwd PATH --global-dir PATH --scope project|global --machine ID
         --model openai/ID --tools a,b --codemode on|only --max-model-calls N
         --json --headless --tui --stream --help
Settings default to project scope. Credentials always use the explicit global directory.
Native code and tools are trusted. No real provider login is performed by help/list/config.
`;
export class Command {
  readonly #options: CommandOptions;
  readonly #output: CommandOutput;
  public constructor(options: CommandOptions = {}) {
    this.#options = options;
    this.#output = new CommandOutput(options);
  }
  public async run(argv: readonly string[]): Promise<number> {
    const lifecycle = new CommandLifecycle(this.#options.signal);
    try {
      return await this.#run(argv, lifecycle);
    } catch (error) {
      this.#output.error(
        lifecycle.signal.aborted
          ? 'Operation cancelled.'
          : error instanceof Error
            ? error.message
            : 'Operation failed',
      );
      return lifecycle.signal.aborted ? 130 : 1;
    } finally {
      lifecycle.close();
    }
  }
  async #run(argv: readonly string[], lifecycle: CommandLifecycle): Promise<number> {
    const line = new CommandLine(
      argv,
      !!this.#options.terminal || !!(process.stdin.isTTY && process.stdout.isTTY),
    );
    if (line.help()) {
      this.#output.write(HELP);
      return 0;
    }
    line.configure();
    const output = new ModelOutput(line, this.#output);
    const app = this.#application(line, output);
    return this.#execute(line, app, output, lifecycle);
  }
  async #execute(
    line: CommandLine,
    app: Zedflow,
    output: ModelOutput,
    lifecycle: CommandLifecycle,
  ): Promise<number> {
    switch (line.command) {
      case 'machines':
        this.#output.print([...(await app.resources.list()).values()]);
        return 0;
      case 'models':
        return this.#models(app);
      case 'config':
        return new ConfigCommand(app, this.#output).run(line);
      case 'auth':
        return new AuthCommand(app.runtime, this.#output, this.#options).run(line, lifecycle);
    }
    if (isInstallationCommand(line.command)) {
      return new InstallationCommand(app, line, this.#output).run();
    }
    if (line.command !== 'run') {
      throw new Error('Unknown command. Use zf --help.');
    }
    return new RunCommand(app, output, this.#options, this.#output).run(line, lifecycle.signal);
  }
  #models(app: Zedflow): number {
    this.#output.print(
      app.runtime.models.getModels().map((model) => ({
        provider: model.provider,
        id: model.id,
        name: model.name,
      })),
    );
    return 0;
  }
  #application(line: CommandLine, output: ModelOutput): Zedflow {
    return new Zedflow({
      cwd: line.cwd,
      globalDirectory: line.globalDirectory,
      overrides: line.overrides,
      ...(this.#options.provider ? { provider: this.#options.provider } : {}),
      ...(this.#options.authContext ? { authContext: this.#options.authContext } : {}),
      onModelEvent: (event) => output.modelEvent(event),
      onToolUpdate: (result) => output.toolUpdate(result),
    });
  }
}
