import { mkdir, mkdtemp } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { RunCoordinator } from '@zf/run';
import { JsonlRunStore } from '@zf/storage';
import { XStateEngine } from '@zf/xstate';
import { NativeEffect } from './native-effect.ts';

type EffectMachineFactory<Module> = (
  effect: NativeEffect,
  increment: number,
  invalid: boolean,
) => Module;

export class ReloadFixture<Module> {
  readonly #effect = new NativeEffect();
  readonly #coordinator: RunCoordinator;
  #id = '';
  #loads = 0;
  #step = 1;
  #invalid = false;

  private constructor(directory: string, machine: EffectMachineFactory<Module>) {
    this.#coordinator = new RunCoordinator({
      engine: new XStateEngine(),
      store: new JsonlRunStore(directory),
      loader: {
        load: async () => {
          this.#loads++;
          return {
            path: 'fixture',
            revision: String(this.#step),
            module: machine(this.#effect, this.#step, this.#invalid),
          };
        },
      },
    });
  }

  public static async create<Module>(
    machine: EffectMachineFactory<Module>,
  ): Promise<ReloadFixture<Module>> {
    const root = resolve('.artifacts/cli-goal/reload-tests');
    await mkdir(root, { recursive: true });
    const directory = await mkdtemp(join(root, 'case-'));
    const fixture = new ReloadFixture(directory, machine);
    fixture.#id = await fixture.#coordinator.launch('fixture');
    return fixture;
  }

  public get coordinator(): RunCoordinator {
    return this.#coordinator;
  }
  public get id(): string {
    return this.#id;
  }
  public done(value: number): void {
    this.#effect.done(value);
  }
  public state(): { calls: number; aborted: boolean; loads: number } {
    return { calls: this.#effect.calls, aborted: this.#effect.aborted, loads: this.#loads };
  }
  public change(): void {
    this.#step = 10;
  }
  public incompatible(): void {
    this.#invalid = true;
  }
}
