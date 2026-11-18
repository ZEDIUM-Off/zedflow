import { TerminalUI } from '@zf/tui';
import type { Zedflow } from './application.ts';
import type { CommandOptions } from './command-options.ts';
import type { CommandLine } from './command-line.ts';
import type { ModelOutput } from './model-output.ts';
import { DeviceIdentity } from './device-identity.ts';

/** Owns the UI's abort subscription; outer run cleanup still stops the UI before its actor. */
export class InteractiveRun {
  readonly #app: Zedflow;
  readonly #options: CommandOptions;
  readonly #output: ModelOutput;
  public constructor(app: Zedflow, options: CommandOptions, output: ModelOutput) {
    this.#app = app;
    this.#options = options;
    this.#output = output;
  }
  public async run(
    id: string,
    machineId: string,
    line: CommandLine,
    signal: AbortSignal,
  ): Promise<number> {
    signal.throwIfAborted();
    const current = this.#create(id, machineId, line.globalDirectory);
    this.#output.attach(current);
    const close = (): void => {
      void current.stop().catch(() => undefined);
    };
    signal.addEventListener('abort', close, { once: true });
    try {
      const closed = current.start();
      if (line.values.prompt !== undefined && line.values.prompt.trim()) {
        this.#app.runs.send(id, { type: 'PROMPT', text: line.values.prompt });
      }
      await closed;
      return signal.aborted ? 130 : 0;
    } finally {
      signal.removeEventListener('abort', close);
    }
  }
  #create(id: string, machineId: string, directory: string): TerminalUI {
    const identity = new DeviceIdentity(directory);
    return new TerminalUI({
      runs: this.#app.runs,
      runId: id,
      machineId,
      models: this.#app.runtime.models,
      loginOptions: { agentName: 'Zedflow', getDeviceId: () => identity.get() },
      ...(this.#options.terminal ? { terminal: this.#options.terminal } : {}),
    });
  }
}
