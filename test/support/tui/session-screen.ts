import type { VirtualTerminal } from '../../../packages/tui/test/virtual-terminal.ts';
import { ENTER } from './keys.ts';

/** Owns scripted keyboard delivery and bounded observation of one real terminal. */
export class SessionScreen {
  readonly #terminal: VirtualTerminal;

  public constructor(terminal: VirtualTerminal) {
    this.#terminal = terminal;
  }

  public get terminal(): VirtualTerminal {
    return this.#terminal;
  }

  public type(text: string): void {
    for (const character of text) {
      this.#terminal.sendInput(character);
    }
  }

  public submit(text: string): void {
    this.type(text);
    this.#terminal.sendInput(ENTER);
  }

  public async screen(): Promise<string> {
    return (await this.#terminal.screen()).join('\n');
  }

  public async waitFor(expected: string): Promise<string> {
    let screen = '';
    for (let attempt = 0; attempt < 100; attempt += 1) {
      screen = await this.screen();
      if (screen.includes(expected)) {
        return screen;
      }
    }
    throw new Error(`Screen never showed ${JSON.stringify(expected)}:\n${screen}`);
  }
}
