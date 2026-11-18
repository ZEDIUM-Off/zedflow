import type {
  AuthPanel,
  SecretInput,
  StateView,
  TranscriptView,
} from '../../../packages/tui/src/index.ts';
import {
  wideAuthPanel,
  wideSecretInput,
  wideStateView,
  wideTranscriptView,
} from './wide-components.ts';

/** Owns the four rendered components and closes the pending panel prompt after width checks. */
export class WidthComponents {
  readonly #panel: AuthPanel;
  readonly #views: (TranscriptView | StateView | SecretInput | AuthPanel)[];

  public constructor(wide: string) {
    const transcript = wideTranscriptView(wide);
    const state = wideStateView(wide);
    const secret = wideSecretInput(wide);
    this.#panel = wideAuthPanel(wide);
    this.#views = [transcript, state, secret, this.#panel];
    const pending = this.#panel.ask(
      { type: 'manual_code', message: wide, placeholder: wide },
      new AbortController().signal,
    );
    pending.catch(() => {});
  }

  public get views(): readonly (TranscriptView | StateView | SecretInput | AuthPanel)[] {
    return this.#views;
  }

  public close(): void {
    this.#panel.close();
  }
}
