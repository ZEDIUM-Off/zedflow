import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { command } from './command.ts';

// Owns the on-disk repository and always runs Git with the fixture's isolated environment.
export class GitRepository {
  readonly #directory: string;

  public constructor(directory: string) {
    this.#directory = directory;
  }

  public async run(args: readonly string[]): Promise<string> {
    return command({ directory: this.#directory, command: 'git', args });
  }

  public async initialize(message: string): Promise<void> {
    await this.run(['init', '--initial-branch=main']);
    await this.commit(message);
  }

  public async commit(message: string): Promise<void> {
    await this.run(['add', '.']);
    await this.run([
      '-c',
      'user.name=Fixture',
      '-c',
      'user.email=fixture@example.invalid',
      'commit',
      '-m',
      message,
    ]);
  }

  public async writeEntryAndCommit(content: string, message: string): Promise<void> {
    await writeFile(join(this.#directory, 'machine.ts'), content);
    await this.commit(message);
  }

  public async installUnsafeHooks(marker: string): Promise<void> {
    const hooks = join(this.#directory, '.git', 'fixture-hooks');
    await mkdir(hooks);
    await writeFile(join(hooks, 'post-checkout'), `#!/bin/sh\ntouch ${JSON.stringify(marker)}\n`, {
      mode: 0o755,
    });
    await this.run(['config', 'core.hooksPath', hooks]);
    await this.run(['config', 'core.fsmonitor', `touch ${marker}`]);
  }
}
