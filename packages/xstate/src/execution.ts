import { createActor } from 'xstate';
import { restoreActor } from 'xstate/persist';
import type { Actor, AnyStateMachine, InspectionEvent } from 'xstate';
import { json } from '@zf/core';
import type { Definition, Event, Execution, Frame, Json } from '@zf/core';

const object = (value: unknown): Record<string, unknown> => {
  if (!value || typeof value !== 'object') {
    throw new Error('Expected object');
  }
  return value as Record<string, unknown>;
};
const inspectionData = (event: InspectionEvent, snapshot: Record<string, unknown>): Json =>
  json({
    id: event.type === '@xstate.actor' ? event.id : undefined,
    snapshot: {
      value: snapshot['value'],
      context: snapshot['context'],
      status: snapshot['status'],
    },
    ...(event.type === '@xstate.transition' ? { event: event.event } : {}),
  });
interface ExecutionOptions {
  readonly definition: Definition;
  readonly observe: (frame: Frame) => void;
  readonly checkpoint?: Json;
  readonly inspect?: (event: InspectionEvent) => void;
}

export class XStateExecution implements Execution {
  readonly #actor: Actor<AnyStateMachine>;
  readonly #observe: (frame: Frame) => void;
  readonly #inspect: ((event: InspectionEvent) => void) | undefined;

  public constructor(options: ExecutionOptions) {
    this.#observe = options.observe;
    this.#inspect = options.inspect;
    const machine = options.definition.opaque as AnyStateMachine;
    const inspect = (event: InspectionEvent): void => this.#onInspection(event);
    this.#actor =
      options.checkpoint === undefined
        ? createActor(machine, { inspect })
        : restoreActor(
            machine,
            options.checkpoint as unknown as Parameters<typeof restoreActor>[1],
            { inspect },
          );
    if (object(this.#actor.getSnapshot())['status'] === 'error') {
      throw new Error('Incompatible checkpoint');
    }
    this.#actor.subscribe({
      error: (error: unknown) =>
        this.#observe({
          type: 'execution.error',
          actor: this.#actor.sessionId,
          data: String(error),
        }),
    });
    this.#actor.on('*', (event: unknown) => this.#onEmission(event));
  }

  public start(): void {
    this.#actor.start();
  }
  public stop(): void {
    this.#actor.stop();
  }
  public send(event: Event): void {
    this.#actor.send(event);
  }

  public view(): Json {
    const snapshot = object(this.#actor.getSnapshot());
    return json({
      value: snapshot['value'],
      context: snapshot['context'],
      status: snapshot['status'],
    });
  }

  public checkpointStatus(): 'ready' | 'busy' | 'unsupported' {
    const snapshot = object(this.#actor.getPersistedSnapshot());
    if (snapshot['status'] !== 'active') {
      return 'unsupported';
    }
    if (
      Object.keys(object(snapshot['children'] ?? {})).length > 0 ||
      (Array.isArray(snapshot['timers']) && snapshot['timers'].length > 0)
    ) {
      return 'busy';
    }
    return 'ready';
  }

  public checkpoint(): Json {
    if (this.checkpointStatus() !== 'ready') {
      throw new Error('Reload supports active child-free, timer-free checkpoints only');
    }
    return json(this.#actor.getPersistedSnapshot());
  }

  #onInspection(event: InspectionEvent): void {
    this.#inspect?.(event);
    let frame: Frame;
    try {
      const snapshot = object(event.snapshot);
      frame = {
        type: event.type,
        actor: event.actorRef.sessionId ?? 'unaddressed',
        ...(event.type === '@xstate.actor' && event.parentRef?.sessionId
          ? { parent: event.parentRef.sessionId }
          : {}),
        data: inspectionData(event, snapshot),
      };
    } catch (error) {
      frame = {
        type: 'serialization.error',
        actor: event.actorRef.sessionId ?? 'unaddressed',
        data: String(error),
      };
    }
    this.#observe(frame);
  }

  #onEmission(event: unknown): void {
    let frame: Frame;
    try {
      frame = { type: 'emitted', actor: this.#actor.sessionId, data: json(event) };
    } catch (error) {
      frame = { type: 'serialization.error', actor: this.#actor.sessionId, data: String(error) };
    }
    this.#observe(frame);
  }
}
