import type { Zedflow } from '../application.ts';
import type { CommandLine } from '../command-line.ts';
import type { CommandOutput } from '../command-output.ts';
import { parseJson } from '../command-json.ts';
export class ConfigCommand {
  readonly #app: Zedflow;
  readonly #output: CommandOutput;
  public constructor(app: Zedflow, output: CommandOutput) {
    this.#app = app;
    this.#output = output;
  }
  public async run(line: CommandLine): Promise<number> {
    if (line.args[0] === 'show') {
      this.#output.print(
        await this.#app.resources.configuration(line.args[1] ?? line.values.machine ?? 'agent'),
      );
    } else {
      await this.#change(line);
    }
    return 0;
  }
  async #change(line: CommandLine): Promise<void> {
    const path = line.args[1];
    if (!path) {
      throw new Error('config requires a settings path');
    }
    const key = line.values.machine ? `machines.${line.values.machine}.${path}` : path;
    if (line.args[0] === 'set' && line.args[2] !== undefined) {
      await this.#app.configuration.set(line.scope, key, parseJson(line.args[2]));
    } else if (line.args[0] === 'unset') {
      await this.#app.configuration.unset(line.scope, key);
    } else {
      throw new Error('Use config show, set or unset');
    }
  }
}
