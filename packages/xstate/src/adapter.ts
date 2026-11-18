import { StateMachine } from 'xstate';
import * as xstate from 'xstate';
import * as persistence from 'xstate/persist';
import type { InspectionEvent } from 'xstate';
import type { Definition, Engine, Execution, Frame, Json } from '@zf/core';
import { XStateExecution } from './execution.ts';

export interface XStateEngineOptions {
  readonly inspect?: (event: InspectionEvent) => void;
}

export class XStateEngine implements Engine {
  public readonly modules: Record<string, unknown> = Object.freeze({
    xstate,
    'xstate/persist': persistence,
  });
  readonly #options: XStateEngineOptions;

  public constructor(options: XStateEngineOptions = {}) {
    this.#options = options;
  }

  public identify(module: unknown): Definition {
    if (!(module instanceof StateMachine)) {
      throw new Error('Export a native machine using the shared XState instance');
    }
    const config: unknown = module.config;
    if (
      !config ||
      typeof config !== 'object' ||
      !('id' in config) ||
      typeof config.id !== 'string' ||
      !('version' in config) ||
      typeof config.version !== 'string'
    ) {
      throw new Error('Machines require a native id and string version');
    }
    return { id: config.id, version: config.version, opaque: module };
  }

  public prepare(
    definition: Definition,
    observe: (frame: Frame) => void,
    checkpoint?: Json,
  ): Execution {
    return new XStateExecution({
      definition,
      observe,
      ...this.#options,
      ...(checkpoint === undefined ? {} : { checkpoint }),
    });
  }
}
