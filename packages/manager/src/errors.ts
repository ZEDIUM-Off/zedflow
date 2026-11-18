/** Publication succeeded. Do not retry the install as though it had rolled back. */
export class CommittedCleanupError extends Error {
  readonly #recoveryDirectory: string;

  public constructor(input: { recoveryDirectory: string; cause: unknown }) {
    super(
      `Installation committed; cleanup pending. Recovery files retained at ${input.recoveryDirectory}`,
      { cause: input.cause },
    );
    this.name = 'CommittedCleanupError';
    this.#recoveryDirectory = input.recoveryDirectory;
  }

  public get committed(): true {
    return true;
  }

  public get recoveryDirectory(): string {
    return this.#recoveryDirectory;
  }
}
