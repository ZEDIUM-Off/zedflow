import type { Definition, Engine, Event, Execution, Frame, Json } from '@zf/core';
import type { ObservationBuffer } from './observation-buffer.ts';

// Preparation can observe synchronously; activation releases those frames before starting.
export class ObservedExecution {
  public readonly definition: Definition;
  readonly #execution: Execution;
  readonly #observations: ObservationBuffer;

  public constructor(
    definition: Definition,
    engine: Engine,
    observations: ObservationBuffer,
    checkpoint?: Json,
  ) {
    this.definition = definition;
    this.#observations = observations;
    const observe = (frame: Frame): void => observations.observe(frame);
    this.#execution =
      checkpoint === undefined
        ? engine.prepare(definition, observe)
        : engine.prepare(definition, observe, checkpoint);
  }

  public start(): void {
    this.#observations.commit();
    this.#execution.start();
  }

  public stop(): void {
    this.#execution.stop();
  }

  public send(event: Event): void {
    this.#execution.send(event);
  }

  public view(): Json {
    return this.#execution.view();
  }

  public checkpointStatus(): 'ready' | 'busy' | 'unsupported' | undefined {
    return this.#execution.checkpointStatus?.();
  }

  public checkpoint(): Json {
    return this.#execution.checkpoint();
  }
}
