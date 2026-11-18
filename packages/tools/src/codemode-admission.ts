/** Reservations include workers awaiting actual exit, not just unsettled scripts. */
export class CodemodeAdmission {
  readonly #maximum: number;
  #active = 0;

  public constructor(maximum: number) {
    this.#maximum = maximum;
  }

  public reserve(): void {
    if (this.#active >= this.#maximum) {
      throw new Error('Code Mode concurrency limit reached; retry after a running script finishes');
    }
    this.#active++;
  }

  public release(): void {
    this.#active--;
  }
}
