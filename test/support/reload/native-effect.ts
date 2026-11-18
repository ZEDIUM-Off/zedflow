import { createAsyncLogic, types } from 'xstate';
import type { AsyncActorLogic, NonReducibleUnknown, EventObject } from 'xstate';

export class NativeEffect {
  #finish: ((value: number) => void) | undefined;
  #calls = 0;
  #aborted = false;

  public get calls(): number {
    return this.#calls;
  }
  public get aborted(): boolean {
    return this.#aborted;
  }

  public get logic(): AsyncActorLogic<number, NonReducibleUnknown, EventObject> & { id?: string } {
    return createAsyncLogic({
      schemas: { output: types<number>() },
      run: ({ signal }) => {
        this.#calls++;
        signal.addEventListener(
          'abort',
          () => {
            this.#aborted = true;
          },
          { once: true },
        );
        return new Promise<number>((resolveEffect) => {
          this.#finish = resolveEffect;
        });
      },
    });
  }

  public done(value: number): void {
    if (!this.#finish) {
      throw new Error('Effect not started');
    }
    this.#finish(value);
  }
}
