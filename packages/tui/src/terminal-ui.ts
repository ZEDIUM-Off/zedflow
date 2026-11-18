import { ProcessTerminal, TuiMainScreen } from '@earendil-works/pi-tui';
import type { Terminal } from '@earendil-works/pi-tui';
import type {
  AssistantMessageEvent,
  LoginOptions,
  Models,
  ToolResultMessage,
} from '@earendil-works/pi-ai';
import type { RunService } from '@zf/core';
import { TerminalCommands } from './terminal-commands.ts';
import { TerminalLogin } from './terminal-login.ts';
import { TerminalNotices } from './terminal-notices.ts';
import { TerminalPanelSlot } from './terminal-panel-slot.ts';
import { TerminalRunLink } from './terminal-run-link.ts';
import { TerminalSession } from './terminal-session.ts';
import { TerminalView } from './terminal-view.ts';

export interface TerminalUIOptions {
  readonly runs: RunService;
  readonly runId: string;
  readonly machineId: string;
  /** Native Pi models; login, logout and status calls go to it unchanged. */
  readonly models: Models;
  /** Defaults to the process terminal. */
  readonly terminal?: Terminal;
  /** Passed to `Models.login`, e.g. the stable device id of OAuth providers. */
  readonly loginOptions?: LoginOptions;
}

/** Pi-native front end for one run; owns UI lifecycle, never the run itself. */
export class TerminalUI {
  readonly #session: TerminalSession;
  readonly #view: TerminalView;
  readonly #notices: TerminalNotices;
  readonly #slot: TerminalPanelSlot;
  readonly #link: TerminalRunLink;
  readonly #commands: TerminalCommands;
  readonly #closed: Promise<void>;
  #phase: 'created' | 'running' | 'closing' = 'created';
  #stopping: Promise<void> | undefined;
  #finishClosed: () => void = () => {};

  public constructor(options: TerminalUIOptions) {
    const tui = new TuiMainScreen(options.terminal ?? new ProcessTerminal());
    this.#view = new TerminalView(tui, options.machineId, options.runId);
    this.#notices = new TerminalNotices(tui, this.#view.agent);
    this.#slot = new TerminalPanelSlot(tui, this.#view.chrome);
    this.#session = new TerminalSession(tui, this.#slot.editor);
    this.#link = new TerminalRunLink(options.runs, options.runId, this.#view, this.#notices);
    const login = new TerminalLogin(
      options.models,
      options.loginOptions,
      this.#notices,
      this.#slot,
    );
    this.#commands = new TerminalCommands(this.#link, this.#slot, this.#notices, login);
    this.#closed = new Promise<void>((resolve) => {
      this.#finishClosed = resolve;
    });
  }

  /** Open and follow the run. Resolves on close; opening failures throw synchronously. */
  public start(): Promise<void> {
    if (this.#phase !== 'created') {
      return this.#closed;
    }
    this.#phase = 'running';
    this.#commands.activate(() => this.#requestClose());
    this.#view.mount(this.#notices.block, this.#slot.bottom);
    this.#session.listen((data) => this.#commands.onInput(data));
    try {
      this.#session.open();
      this.#link.subscribe(
        () => this.#requestClose(),
        () => this.#phase !== 'closing',
      );
    } catch (error) {
      this.#failedStart();
      throw error instanceof Error ? error : new Error(String(error));
    }
    return this.#closed;
  }

  /** Cancel and settle UI work, then restore the terminal. Idempotent; never stops the run. */
  public stop(): Promise<void> {
    this.#stopping ??= this.#shutdown();
    return this.#stopping;
  }

  public modelEvent(event: AssistantMessageEvent): void {
    if (this.#phase !== 'closing') {
      this.#view.modelEvent(event);
    }
  }

  public toolUpdate(result: ToolResultMessage): void {
    if (this.#phase !== 'closing') {
      this.#view.toolUpdate(result);
    }
  }

  #failedStart(): void {
    this.#phase = 'closing';
    this.#commands.disable();
    this.#notices.close();
    this.#link.close();
    this.#session.detach();
    this.#session.restore();
    this.#finishClosed();
  }

  async #shutdown(): Promise<void> {
    const wasRunning = this.#phase === 'running';
    this.#phase = 'closing';
    this.#commands.close();
    this.#link.close();
    this.#session.detach();
    this.#slot.close();
    try {
      await this.#commands.settle();
      if (wasRunning) {
        await this.#session.drain();
      }
    } finally {
      this.#session.restore();
      this.#finishClosed();
    }
  }

  #requestClose(): void {
    this.stop().catch(() => {});
  }
}
