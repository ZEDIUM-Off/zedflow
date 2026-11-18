import { copyFile, lstat, mkdir, readdir } from 'node:fs/promises';
import { dirname, join } from 'node:path';

/**
 * One copy of a source tree. It owns the excluded root, such as a destination inside the source,
 * and refuses anything that is not a real file or directory.
 */
export class SourceCopy {
  readonly #excludedRoot: string | undefined;

  public constructor(excludedRoot: string | undefined) {
    this.#excludedRoot = excludedRoot;
  }

  public async copy(from: string, to: string): Promise<void> {
    if (from === this.#excludedRoot) {
      return;
    }
    const info = await lstat(from);
    if (info.isSymbolicLink() || (!info.isDirectory() && !info.isFile())) {
      throw new Error(`Source must contain only real files/directories: ${from}`);
    }
    if (info.isDirectory()) {
      await this.#copyDirectory(from, to);
    } else {
      await mkdir(dirname(to), { recursive: true });
      await copyFile(from, to);
    }
  }

  async #copyDirectory(from: string, to: string): Promise<void> {
    await mkdir(to, { recursive: true });
    for (const name of (await readdir(from)).sort()) {
      if (name !== '.git' && name !== 'node_modules') {
        await this.copy(join(from, name), join(to, name));
      }
    }
  }
}
