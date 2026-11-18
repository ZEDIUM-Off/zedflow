import type { AuthPanel } from './auth-panel.ts';
import { TuiAuthInteraction } from './tui-auth-interaction.ts';
import type { TerminalOperationScope } from './terminal-operation-scope.ts';
import type { TerminalPanelSlot } from './terminal-panel-slot.ts';

/** Owns one login's prompt surface and cancellation registration until native login unwinds. */
export class TerminalLoginAttempt {
  readonly #scope: TerminalOperationScope;
  readonly #slot: TerminalPanelSlot;
  readonly #controller: AbortController;
  #panel: AuthPanel | undefined;

  public constructor(scope: TerminalOperationScope, slot: TerminalPanelSlot) {
    this.#scope = scope;
    this.#slot = slot;
    this.#controller = scope.controller();
  }

  public get cancelled(): boolean {
    return this.#controller.signal.aborted;
  }

  public open(providerName: string): TuiAuthInteraction {
    const panel = this.#slot.create(this.#controller, providerName);
    this.#panel = panel;
    const interaction = new TuiAuthInteraction({ panel, signal: this.#controller.signal });
    this.#slot.mount(panel);
    return interaction;
  }

  public cancel(): void {
    this.#controller.abort();
  }

  public close(): void {
    this.#panel?.close();
    this.#scope.release(this.#controller);
  }
}
