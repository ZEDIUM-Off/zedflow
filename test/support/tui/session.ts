import type { AnyStateMachine } from 'xstate';
import { TerminalUI } from '../../../packages/tui/src/index.ts';
import type { AuthFixture, RunFixture } from '../../../packages/tui/test/fixtures.ts';
import { createAuthFixture, createRun } from '../../../packages/tui/test/fixtures.ts';
import { VirtualTerminal } from '../../../packages/tui/test/virtual-terminal.ts';
import { SessionScreen } from './session-screen.ts';

export interface SessionOptions {
  readonly columns?: number;
  readonly rows?: number;
  readonly slowLoad?: Promise<void>;
}

/** Owns one native UI lifecycle; the test still explicitly decides when to stop its run. */
export class Session {
  readonly #run: RunFixture;
  readonly #auth: AuthFixture;
  readonly #screen: SessionScreen;
  readonly #ui: TerminalUI;
  readonly #done: Promise<void>;

  private constructor(
    machineId: string,
    run: RunFixture,
    auth: AuthFixture,
    terminal: VirtualTerminal,
  ) {
    this.#run = run;
    this.#auth = auth;
    this.#screen = new SessionScreen(terminal);
    this.#ui = new TerminalUI({
      runs: run.runs,
      runId: run.runId,
      machineId,
      models: auth.models,
      terminal,
      loginOptions: { getDeviceId: () => '00000000-0000-4000-8000-000000000000' },
    });
    this.#done = this.#ui.start();
  }

  public static async open(
    machineId: string,
    machine: (load: number) => AnyStateMachine,
    options: SessionOptions = {},
  ): Promise<Session> {
    const run = await createRun(machine, options.slowLoad ? { slowLoad: options.slowLoad } : {});
    const auth = createAuthFixture();
    const terminal = new VirtualTerminal(options.columns ?? 80, options.rows ?? 30);
    return new Session(machineId, run, auth, terminal);
  }

  public get run(): RunFixture {
    return this.#run;
  }
  public get auth(): AuthFixture {
    return this.#auth;
  }
  public get terminal(): VirtualTerminal {
    return this.#screen.terminal;
  }
  public get screen(): SessionScreen {
    return this.#screen;
  }
  public get ui(): TerminalUI {
    return this.#ui;
  }
  public get done(): Promise<void> {
    return this.#done;
  }

  public async finish(): Promise<void> {
    await this.#ui.stop();
    await this.#done;
  }
}
