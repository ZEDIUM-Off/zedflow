import type { Json } from '@zf/core';
import type { Zedflow } from '../application.ts';
import type { CommandOptions } from '../command-options.ts';
import type { CommandLine } from '../command-line.ts';
import type { CommandOutput } from '../command-output.ts';
import type { ModelOutput } from '../model-output.ts';
import { object, parseJson, validEvent } from '../command-json.ts';
import { finalText, runFailure } from '../turn-result.ts';
import { HeadlessTurn } from '../headless-turn.ts';
import { InteractiveRun } from '../interactive-run.ts';

export class RunCommand {
  readonly #app: Zedflow;
  readonly #modelOutput: ModelOutput;
  readonly #output: CommandOutput;
  readonly #interactive: InteractiveRun;
  public constructor(
    app: Zedflow,
    modelOutput: ModelOutput,
    options: CommandOptions,
    output: CommandOutput,
  ) {
    this.#app = app;
    this.#modelOutput = modelOutput;
    this.#output = output;
    this.#interactive = new InteractiveRun(app, options, modelOutput);
  }
  public async run(line: CommandLine, signal: AbortSignal): Promise<number> {
    if (line.values.prompt !== undefined && !line.values.prompt.trim()) {
      throw new Error('Prompt must not be empty');
    }
    const machineId = line.args[0] ?? line.values.machine ?? 'agent';
    const id = await this.#app.launch(machineId);
    try {
      this.#events(id, line.values.event ?? []);
      return line.interactive()
        ? await this.#interactive.run(id, machineId, line, signal)
        : await this.#headless(id, machineId, line, signal);
    } finally {
      try {
        await this.#modelOutput.stop();
      } finally {
        this.#app.runs.stop(id);
      }
    }
  }
  #events(id: string, events: readonly string[]): void {
    for (const text of events) {
      const event = parseJson(text);
      if (!validEvent(event)) {
        throw new Error('Event requires a string type');
      }
      this.#app.runs.send(id, event);
    }
  }
  async #headless(
    id: string,
    machineId: string,
    line: CommandLine,
    signal: AbortSignal,
  ): Promise<number> {
    let view: Json;
    if (line.values.prompt !== undefined) {
      if (!line.values.prompt.trim()) {
        throw new Error('Prompt must not be empty');
      }
      await this.#requireAuth(machineId, signal);
      this.#app.runs.send(id, { type: 'PROMPT', text: line.values.prompt });
      view = await new HeadlessTurn(this.#app.runs, id, signal).wait();
    } else {
      view = this.#app.runs.view(id);
    }
    return this.#result(id, view, line);
  }
  async #requireAuth(machineId: string, signal: AbortSignal): Promise<void> {
    const configuration = await this.#app.resources.configuration(machineId);
    const selector = configuration.settings['model'];
    if (
      selector &&
      object(selector) &&
      typeof selector['provider'] === 'string' &&
      typeof selector['id'] === 'string'
    ) {
      await this.#app.runtime.requireAuth(
        this.#app.runtime.select(selector['provider'], selector['id']),
        { signal },
      );
    }
  }
  #result(id: string, view: Json, line: CommandLine): number {
    if (line.values.json || line.values.prompt === undefined) {
      this.#output.print(view);
    } else if (!line.values.stream) {
      this.#output.write(`${finalText(view)}\n`);
    }
    const failure = runFailure(view);
    if (failure) {
      this.#output.error(`${failure}\nRun: ${id}`);
      return 1;
    }
    return 0;
  }
}
