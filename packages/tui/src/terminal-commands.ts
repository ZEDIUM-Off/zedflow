import { matchesKey } from '@earendil-works/pi-tui';
import type { TuiInputListenerResult } from '@earendil-works/pi-tui';
import type { Event } from '@zf/core';
import type { TerminalLogin } from './terminal-login.ts';
import { GENERIC_HINT } from './terminal-notices.ts';
import type { TerminalNotices } from './terminal-notices.ts';
import { TerminalOperationScope } from './terminal-operation-scope.ts';
import type { TerminalPanelSlot } from './terminal-panel-slot.ts';
import type { TerminalRunLink } from './terminal-run-link.ts';

const HELP = [
  '/help                      show this list',
  '/reload                    reload the machine once the active work finishes',
  '/login [api-key|oauth]     sign in through the native provider flow',
  '/logout                    remove the stored credential',
  '/auth                      show sign-in status',
  '/reset                     send RESET to the machine',
  '/quit                      close the terminal UI',
];
const isEvent = (value: unknown): value is Event =>
  typeof value === 'object' &&
  value !== null &&
  !Array.isArray(value) &&
  'type' in value &&
  typeof value.type === 'string';

/** Owns input admission, command routing and the scope of pending UI commands. */
export class TerminalCommands {
  readonly #link: TerminalRunLink;
  readonly #slot: TerminalPanelSlot;
  readonly #notices: TerminalNotices;
  readonly #login: TerminalLogin;
  readonly #scope: TerminalOperationScope;
  #requestClose: (() => void) | undefined;

  public constructor(
    link: TerminalRunLink,
    slot: TerminalPanelSlot,
    notices: TerminalNotices,
    login: TerminalLogin,
  ) {
    this.#link = link;
    this.#slot = slot;
    this.#notices = notices;
    this.#login = login;
    this.#scope = new TerminalOperationScope(notices);
    slot.editor.onSubmit = (text) => this.#submit(text);
  }

  public activate(requestClose: () => void): void {
    this.#requestClose = requestClose;
  }

  public disable(): void {
    this.#requestClose = undefined;
  }

  public close(): void {
    this.disable();
    this.#notices.close();
    this.#scope.abort();
  }

  public settle(): Promise<void> {
    return this.#scope.settle();
  }

  public onInput(data: string): TuiInputListenerResult {
    if (matchesKey(data, 'ctrl+c')) {
      this.#interrupt();
      return { consume: true };
    }
    if (this.#login.active) {
      return undefined;
    }
    if (matchesKey(data, 'escape') && this.#link.active) {
      this.#link.send({ type: 'CANCEL' });
      return { consume: true };
    }
    if (matchesKey(data, 'ctrl+d') && this.#slot.editor.getText() === '') {
      this.#requestClose?.();
      return { consume: true };
    }
    return undefined;
  }

  #interrupt(): void {
    if (this.#login.active) {
      this.#login.cancel();
    } else if (this.#link.active) {
      this.#link.send({ type: 'CANCEL' });
    } else {
      this.#requestClose?.();
    }
  }

  #submit(raw: string): void {
    const text = raw.trim();
    if (text === '' || !this.#requestClose) {
      return;
    }
    if (text.startsWith('/')) {
      const [name = '', ...args] = text.slice(1).split(/\s+/u);
      if (['quit', 'exit'].includes(name.toLowerCase())) {
        this.#requestClose?.();
        return;
      }
      this.#scope.track(this.#command(name.toLowerCase(), args));
      return;
    }
    this.#slot.editor.addToHistory(text);
    this.#sendText(text);
  }

  #sendText(text: string): void {
    if (!this.#link.agent) {
      this.#sendJson(text);
    } else if (this.#link.active) {
      this.#notices.add('The agent is busy. Press Escape to cancel the active turn.');
    } else {
      this.#link.send({ type: 'PROMPT', text });
    }
  }

  #sendJson(text: string): void {
    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch {
      this.#notices.add(`Input is not JSON. ${GENERIC_HINT}`);
      return;
    }
    if (isEvent(parsed)) {
      this.#link.send(parsed);
    } else {
      this.#notices.add(`The event needs a string "type". ${GENERIC_HINT}`);
    }
  }

  async #command(name: string, args: readonly string[]): Promise<void> {
    switch (name) {
      case 'help':
        this.#notices.replace(HELP);
        return;
      case 'reload':
        return this.#link.reload(this.#scope);
      case 'login':
        return this.#login.signIn(args, this.#scope);
      case 'logout':
        return this.#login.signOut(args, this.#scope);
      case 'auth':
        return this.#login.status(this.#scope);
      case 'reset':
        if (this.#link.reset()) {
          this.#notices.add('Reset sent.');
        }
        return;
      default:
        this.#notices.add(`Unknown command /${name}. Try /help.`);
    }
  }
}
