import { Container, Editor } from '@earendil-works/pi-tui';
import type { TUI } from '@earendil-works/pi-tui';
import { AuthPanel } from './auth-panel.ts';
import { editorTheme } from './style.ts';
import type { TerminalChrome } from './terminal-chrome.ts';

/** Keeps exactly one editor or authentication panel mounted and focused. */
export class TerminalPanelSlot {
  public readonly bottom = new Container();
  public readonly editor: Editor;
  readonly #tui: TUI;
  readonly #chrome: TerminalChrome;
  #panel: AuthPanel | undefined;

  public constructor(tui: TUI, chrome: TerminalChrome) {
    this.#tui = tui;
    this.#chrome = chrome;
    this.editor = new Editor(tui, editorTheme);
    this.bottom.addChild(this.editor);
  }

  public focus(): void {
    this.#tui.setFocus(this.editor);
  }

  public create(controller: AbortController, providerName: string): AuthPanel {
    const panel = new AuthPanel({
      requestRender: () => this.#tui.requestRender(),
      onCancel: () => controller.abort(),
    });
    panel.setTitle(`Sign in to ${providerName}`);
    return panel;
  }

  public mount(panel: AuthPanel): void {
    this.#panel = panel;
    this.bottom.clear();
    this.bottom.addChild(panel);
    this.#tui.setFocus(panel);
    this.#tui.requestRender();
    this.#chrome.login(true);
  }

  public hide(): void {
    this.#panel = undefined;
    this.bottom.clear();
    this.bottom.addChild(this.editor);
    this.focus();
    this.#tui.requestRender();
    this.#chrome.login(false);
  }

  public close(): void {
    this.#panel?.close();
  }
}
