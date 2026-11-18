import { randomUUID } from 'node:crypto';
import { json } from '@zf/core';
import type { Definition, Engine, Event, Json, MachineSource, RecordStore } from '@zf/core';
import { ObservationBuffer } from './observation-buffer.ts';
import { ObservedExecution } from './observed-execution.ts';

const requireIdentity = (previous: Definition, candidate: Definition): void => {
  if (candidate.id !== previous.id || candidate.version !== previous.version) {
    throw new Error('Reload requires unchanged native machine identity/version');
  }
};
const opened = (source: MachineSource, definition: Definition, origin?: string): Json =>
  json({
    path: source.path,
    revision: source.revision,
    machine: definition.id,
    version: definition.version,
    origin,
  });
const replacement = (
  source: MachineSource,
  previous: MachineSource,
  generation: number,
  checkpoint: Json,
): Json =>
  json({ generation, previousRevision: previous.revision, revision: source.revision, checkpoint });

// Owns the active native generation and the journal boundary that precedes its activation.
export class RunExecution {
  public readonly id: string;
  readonly #engine: Engine;
  readonly #store: RecordStore;
  readonly #notify: (runId: string) => void;
  #source: MachineSource;
  #current: ObservedExecution;
  #generation = 1;

  public constructor(
    source: MachineSource,
    engine: Engine,
    store: RecordStore,
    notify: (runId: string) => void,
  ) {
    const definition = engine.identify(source.module);
    this.id = randomUUID();
    this.#engine = engine;
    this.#store = store;
    this.#notify = notify;
    this.#source = source;
    this.#current = new ObservedExecution(
      definition,
      engine,
      new ObservationBuffer({
        store,
        runId: this.id,
        generation: 1,
        notify: () => notify(this.id),
      }),
    );
  }

  public get path(): string {
    return this.#source.path;
  }

  public open(origin?: string): void {
    this.#store.append(this.id, 'opened', opened(this.#source, this.#current.definition, origin));
  }

  public start(): void {
    this.#current.start();
  }

  public send(event: Event): void {
    this.#store.append(this.id, 'input', json({ generation: this.#generation, event }));
    this.#current.send(event);
  }

  public view(): Json {
    return this.#current.view();
  }

  public get generation(): number {
    return this.#generation;
  }

  public stop(markStopped: () => void): void {
    this.#current.stop();
    markStopped();
    this.#store.append(this.id, 'closed', json({ generation: this.#generation }));
    this.#notify(this.id);
  }

  public checkpointStatus(): 'ready' | 'busy' | 'unsupported' | undefined {
    return this.#current.checkpointStatus();
  }

  public checkpoint(): Json {
    return this.#current.checkpoint();
  }

  public replace(source: MachineSource): void {
    const definition = this.#engine.identify(source.module);
    requireIdentity(this.#current.definition, definition);
    const checkpoint = this.#current.checkpoint();
    const generation = this.#generation + 1;
    const observations = new ObservationBuffer({
      store: this.#store,
      runId: this.id,
      generation,
      notify: () => this.#notify(this.id),
    });
    const candidate = new ObservedExecution(definition, this.#engine, observations, checkpoint);
    // Storage failure must leave the previous actor usable. Its stop frames retain its generation.
    this.#store.append(
      this.id,
      'replacement',
      replacement(source, this.#source, generation, checkpoint),
    );
    this.#current.stop();
    this.#source = source;
    this.#current = candidate;
    this.#generation = generation;
    candidate.start();
  }
}
