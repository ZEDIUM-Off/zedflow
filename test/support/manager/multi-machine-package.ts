import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { createMachine } from './machine-files.ts';

// Owns the two-machine source tree and its shared import/dependency closure.
export class MultiMachinePackage {
  readonly #directory: string;

  public constructor(directory: string) {
    this.#directory = join(directory, 'multi-package');
  }

  public get directory(): string {
    return this.#directory;
  }

  public async create(marker: string): Promise<void> {
    await createMachine({
      directory: join(this.#directory, 'machines', 'team', 'alpha'),
      name: 'alpha',
      entry: "import { shared } from '../../../shared.ts'; export const result = shared;",
    });
    await createMachine({
      directory: join(this.#directory, 'machines', 'team', 'beta'),
      name: 'beta',
    });
    await writeFile(
      join(this.#directory, 'package.json'),
      JSON.stringify({
        type: 'module',
        dependencies: { 'fixture-helper': '^1.0.0' },
        scripts: { prepare: `touch ${marker}` },
      }),
    );
    await this.writeShared('source-v1');
  }

  public async writeShared(value: string): Promise<void> {
    await writeFile(
      join(this.#directory, 'shared.ts'),
      `import { value } from 'fixture-helper'; export const shared = '${value}/' + value;`,
    );
  }
}
