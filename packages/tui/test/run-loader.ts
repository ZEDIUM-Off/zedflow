import type { Loader, MachineSource } from '@zf/core';
import type { AnyStateMachine } from 'xstate';

/** Counts native loads and delays reloads, but never the initial launch. */
export class RunLoader implements Loader {
  readonly #machine: (load: number) => AnyStateMachine;
  readonly #options: { readonly slowLoad?: Promise<void> };
  #loads = 0;

  public constructor(
    machine: (load: number) => AnyStateMachine,
    options: { readonly slowLoad?: Promise<void> },
  ) {
    this.#machine = machine;
    this.#options = options;
  }

  public get loads(): number {
    return this.#loads;
  }

  public async load(): Promise<MachineSource> {
    this.#loads += 1;
    if (this.#loads > 1 && this.#options.slowLoad) {
      await this.#options.slowLoad;
    }
    const machine = this.#machine;
    return { path: 'fixture', revision: String(this.#loads), module: machine(this.#loads) };
  }
}
