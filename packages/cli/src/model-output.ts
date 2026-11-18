import type { AssistantMessageEvent, ToolResultMessage } from '@earendil-works/pi-ai';
import type { TerminalUI } from '@zf/tui';
import type { CommandLine } from './command-line.ts';
import type { CommandOutput } from './command-output.ts';

/** Routes model/tool updates to a live UI, otherwise to the requested headless stream. */
export class ModelOutput {
  readonly #line: CommandLine;
  readonly #output: CommandOutput;
  #ui: TerminalUI | undefined;
  public constructor(line: CommandLine, output: CommandOutput) {
    this.#line = line;
    this.#output = output;
  }
  public attach(ui: TerminalUI): void {
    this.#ui = ui;
  }
  public modelEvent(event: AssistantMessageEvent): void {
    this.#ui?.modelEvent(event);
    if (
      !this.#ui &&
      this.#line.values.stream &&
      !this.#line.values.json &&
      event.type === 'text_delta'
    ) {
      this.#output.write(event.delta);
    }
  }
  public toolUpdate(result: ToolResultMessage): void {
    this.#ui?.toolUpdate(result);
  }
  public async stop(): Promise<void> {
    await this.#ui?.stop();
  }
}
