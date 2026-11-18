import type { Editor, TUI, TuiInputListenerResult } from '@earendil-works/pi-tui';

/** Owns terminal restoration and the lifetime of its input listener. */
export class TerminalSession {
  readonly #tui: TUI;
  readonly #editor: Editor;
  #started = false;
  #removeInputListener: (() => void) | undefined;

  public constructor(tui: TUI, editor: Editor) {
    this.#tui = tui;
    this.#editor = editor;
  }

  public listen(listener: (data: string) => TuiInputListenerResult): void {
    this.#tui.setFocus(this.#editor);
    this.#removeInputListener = this.#tui.addInputListener(listener);
  }

  public open(): void {
    this.#tui.start();
    this.#started = true;
  }

  public detach(): void {
    this.#removeInputListener?.();
    this.#removeInputListener = undefined;
  }

  public async drain(): Promise<void> {
    if (this.#started) {
      await this.#tui.terminal.drainInput(1000);
    }
  }

  public restore(): void {
    if (this.#started) {
      this.#started = false;
      this.#tui.stop();
    }
  }
}
