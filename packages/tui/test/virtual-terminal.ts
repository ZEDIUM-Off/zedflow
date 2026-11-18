// Adapted from Pi 1.5.2 (MIT), copyright Mario Zechner and Earendil Works.
// Source: packages/tui/test/virtual-terminal.ts at revision b4afdddf44e059c67ccf5097e5317d913521b470;
// see ../PROVENANCE.json. Adaptation: project style, recorded writes and titles, lifecycle counters.
import xterm from '@xterm/headless';
import type { Terminal } from '@earendil-works/pi-tui';
import type { Terminal as XtermTerminal } from '@xterm/headless';

/** Native Pi `Terminal` over xterm.js with a scripted keyboard and observable side effects. */
export class VirtualTerminal implements Terminal {
  public readonly kittyProtocolActive = true;
  /** Raw bytes the renderer wrote, in order. */
  public readonly writes: string[] = [];
  /** Window titles set through OSC 0/2, from the application or from injected text. */
  public readonly titles: string[] = [];
  public starts = 0;
  public stops = 0;
  public drains = 0;
  readonly #xterm: XtermTerminal;
  #onInput: ((data: string) => void) | undefined;
  #onResize: (() => void) | undefined;
  #columns: number;
  #rows: number;

  public constructor(columns = 80, rows = 24) {
    this.#columns = columns;
    this.#rows = rows;
    this.#xterm = new xterm.Terminal({
      cols: columns,
      rows,
      disableStdin: true,
      allowProposedApi: true,
    });
    this.#xterm.onTitleChange((title) => this.titles.push(title));
  }

  public get columns(): number {
    return this.#columns;
  }

  public get rows(): number {
    return this.#rows;
  }

  public get running(): boolean {
    return this.#onInput !== undefined;
  }

  public start(onInput: (data: string) => void, onResize: () => void): void {
    this.starts += 1;
    this.#onInput = onInput;
    this.#onResize = onResize;
    this.write('\u001b[?2004h');
  }

  public stop(): void {
    this.stops += 1;
    this.write('\u001b[?2004l');
    this.#onInput = undefined;
    this.#onResize = undefined;
  }

  public drainInput(): Promise<void> {
    this.drains += 1;
    return Promise.resolve();
  }

  public write(data: string): void {
    this.writes.push(data);
    this.#xterm.write(data);
  }

  public moveBy(lines: number): void {
    if (lines > 0) {
      this.write(`\u001b[${lines}B`);
    } else if (lines < 0) {
      this.write(`\u001b[${-lines}A`);
    }
  }

  public hideCursor(): void {
    this.write('\u001b[?25l');
  }

  public showCursor(): void {
    this.write('\u001b[?25h');
  }

  public clearLine(): void {
    this.write('\u001b[K');
  }

  public clearFromCursor(): void {
    this.write('\u001b[J');
  }

  public clearScreen(): void {
    this.write('\u001b[2J\u001b[H');
  }

  public setTitle(title: string): void {
    this.write(`\u001b]0;${title}\u0007`);
  }

  public setProgress(): void {}

  public setProgramStatus(): void {}

  /** Deliver keyboard input as the terminal would. */
  public sendInput(data: string): void {
    this.#onInput?.(data);
  }

  public resize(columns: number, rows: number): void {
    this.#columns = columns;
    this.#rows = rows;
    this.#xterm.resize(columns, rows);
    this.#onResize?.();
  }

  /** Wait for the throttled renderer and the emulator, then return the visible screen rows. */
  public async screen(): Promise<string[]> {
    await new Promise<void>((resolve) => process.nextTick(resolve));
    await new Promise<void>((resolve) => setTimeout(resolve, 30));
    await new Promise<void>((resolve) => this.#xterm.write('', resolve));
    return this.#rowsOf(this.#xterm.buffer.active.viewportY, this.#xterm.rows);
  }

  /** Every row, scrollback included. */
  public async scrollback(): Promise<string[]> {
    await this.screen();
    return this.#rowsOf(0, this.#xterm.buffer.active.length);
  }

  #rowsOf(start: number, count: number): string[] {
    const rows: string[] = [];
    for (let index = 0; index < count; index += 1) {
      rows.push(this.#xterm.buffer.active.getLine(start + index)?.translateToString(true) ?? '');
    }
    return rows;
  }
}
