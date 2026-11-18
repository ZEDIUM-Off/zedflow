import { homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { validResourceId } from '@zf/capabilities';
import type { Settings } from '@zf/config';
import { parseArguments } from './command-arguments.ts';
import { commandOverrides } from './command-overrides.ts';

/** Parses once. Early usage validation deliberately precedes help; settings validation does not. */
export class CommandLine {
  public readonly values: ReturnType<typeof parseArguments>['values'];
  public readonly args: readonly string[];
  public readonly command: string | undefined;
  public readonly terminalAvailable: boolean;
  #scope: 'global' | 'project' = 'project';
  #overrides: Settings = {};
  #cwd = '';
  #globalDirectory = '';
  public constructor(argv: readonly string[], terminalAvailable: boolean) {
    const { values, positionals } = parseArguments(argv);
    this.values = values;
    const [requested, ...args] = positionals;
    this.args = args;
    this.terminalAvailable = terminalAvailable;
    this.command =
      requested ?? (values.prompt !== undefined || terminalAvailable ? 'run' : undefined);
    this.#validateUsage();
  }
  public help(): boolean {
    return !!this.values.help || this.command === 'help' || !this.command;
  }
  public configure(): void {
    this.#cwd = resolve(this.values.cwd ?? process.cwd());
    this.#globalDirectory = resolve(this.values['global-dir'] ?? join(homedir(), '.zedflow'));
    const scope = this.values.scope ?? 'project';
    if (scope !== 'project' && scope !== 'global') {
      throw new Error('Scope must be project or global');
    }
    this.#scope = scope;
    this.#overrides = commandOverrides(this.values);
  }
  public get cwd(): string {
    return this.#cwd;
  }
  public get globalDirectory(): string {
    return this.#globalDirectory;
  }
  public get scope(): 'global' | 'project' {
    return this.#scope;
  }
  public get overrides(): Readonly<Settings> {
    return this.#overrides;
  }
  public interactive(): boolean {
    return (
      !!this.values.tui ||
      (!this.values.headless &&
        this.values.prompt === undefined &&
        !this.values.event?.length &&
        this.terminalAvailable)
    );
  }
  #validateUsage(): void {
    if (this.values.machine && !validResourceId(this.values.machine)) {
      throw new Error('Invalid qualified machine ID');
    }
    if (this.values.tui && this.values.headless) {
      throw new Error('--tui and --headless cannot be combined');
    }
    if (this.values.tui && !this.terminalAvailable) {
      throw new Error('Interactive mode requires a terminal');
    }
  }
}
