import type { Zedflow } from '../application.ts';
import type { CommandLine } from '../command-line.ts';
import type { CommandOutput } from '../command-output.ts';
import { installationSource } from '../installation-source.ts';

export const isInstallationCommand = (command: string | undefined): boolean =>
  ['install', 'update', 'remove', 'installed', 'restore'].includes(command ?? '');

/** One invocation keeps its selected scope through validation, resolution and receipt output. */
export class InstallationCommand {
  readonly #manager: ReturnType<Zedflow['manager']>;
  readonly #line: CommandLine;
  readonly #output: CommandOutput;
  public constructor(app: Zedflow, line: CommandLine, output: CommandOutput) {
    this.#manager = app.manager(line.scope);
    this.#line = line;
    this.#output = output;
  }
  public async run(): Promise<number> {
    if (this.#line.command === 'installed') {
      this.#output.print(await this.#manager.list());
    } else if (this.#line.command === 'restore') {
      await this.#restore();
    } else {
      await this.#change();
    }
    return 0;
  }
  async #restore(): Promise<void> {
    if (!this.#line.values.frozen) {
      throw new Error('restore requires --frozen');
    }
    this.#output.print(await this.#manager.restore({ frozen: true }));
  }
  async #change(): Promise<void> {
    const target = this.#line.args[0];
    if (!target) {
      throw new Error('Installation command requires a source or ID');
    }
    if (this.#line.command === 'remove') {
      await this.#manager.remove(target);
    } else {
      this.#output.print(
        this.#line.command === 'install'
          ? await this.#manager.install(installationSource(this.#line.cwd, target))
          : await this.#manager.update(target),
      );
    }
  }
}
