import { dirname } from 'node:path';
import type { InstallerProcess } from './process.ts';
import type { Source } from './types.ts';

/** A detached checkout in staging; resolution is validated before checkout and never executes source. */
export class GitCheckout {
  readonly #directory: string;
  readonly #process: InstallerProcess;

  public constructor(directory: string, process: InstallerProcess) {
    this.#directory = directory;
    this.#process = process;
  }

  public async prepare(source: Source, frozenRevision?: string): Promise<string> {
    const local = !/^https?:\/\//.test(source.location);
    await this.#process.git(
      [
        'clone',
        ...(local ? ['--local', '--no-hardlinks'] : []),
        '--no-checkout',
        '--',
        source.location,
        this.#directory,
      ],
      dirname(this.#directory),
    );
    const commit = await this.#resolve(frozenRevision ?? source.ref, frozenRevision !== undefined);
    if (!/^[a-f0-9]{40,64}$/.test(commit)) {
      throw new Error('Git did not resolve a full commit');
    }
    await this.#process.git(['checkout', '--detach', '--force', commit], this.#directory);
    return commit;
  }

  async #resolve(revision: string, frozen: boolean): Promise<string> {
    try {
      return await this.#commit(revision);
    } catch (error) {
      if (frozen || revision.startsWith('refs/')) {
        throw error;
      }
      // clone creates non-default branches under refs/remotes/origin, not refs/heads.
      return this.#commit(`refs/remotes/origin/${revision}`);
    }
  }

  async #commit(revision: string): Promise<string> {
    return (
      await this.#process.git(
        ['rev-parse', '--verify', '--end-of-options', `${revision}^{commit}`],
        this.#directory,
      )
    ).trim();
  }
}
