import type { CommandOptions } from './command-options.ts';
import { terminalText } from './terminal-text.ts';

export class CommandOutput {
  readonly #options: CommandOptions;
  public constructor(options: CommandOptions) {
    this.#options = options;
  }
  public write(text: string): void {
    (
      this.#options.write ??
      ((value) => {
        process.stdout.write(value);
      })
    )(terminalText(text));
  }
  public error(text: string): void {
    (
      this.#options.error ??
      ((value) => {
        process.stderr.write(value);
      })
    )(`${terminalText(text)}\n`);
  }
  public print(value: unknown): void {
    this.write(
      `${JSON.stringify(value, null, 2).replace(/[\u007f-\u009f]/g, (char) => `\\u${char.charCodeAt(0).toString(16).padStart(4, '0')}`)}\n`,
    );
  }
}
