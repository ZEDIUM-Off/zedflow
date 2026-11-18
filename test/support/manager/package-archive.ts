import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';
import type { Json } from '@zf/core';
import { command } from './command.ts';
import { tarEntry } from './tar-entry.ts';
import type { TarEntry } from './tar-entry.ts';

export interface PublishedPackage {
  readonly name: string;
  readonly version: string;
  readonly files: Readonly<Record<string, string>>;
  readonly metadata?: Readonly<Record<string, Json>>;
  readonly archiveEntries?: readonly TarEntry[];
}

// Owns a published version's on-disk package tree, archive and common manifest.
// The enclosing manager fixture removes the registry directory after the server closes.
export class PackageArchive {
  readonly #root: string;
  readonly #package: PublishedPackage;
  readonly #manifest: Readonly<Record<string, Json>>;

  public constructor(directory: string, input: PublishedPackage) {
    this.#root = join(directory, `${input.name.replaceAll('/', '-')}-${input.version}`);
    this.#package = input;
    this.#manifest = {
      name: input.name,
      version: input.version,
      type: 'module',
      ...input.metadata,
    };
  }

  public async build(): Promise<Buffer> {
    await this.#writeFiles();
    const archive = join(this.#root, 'package.tgz');
    await command({
      directory: this.#root,
      command: 'tar',
      args: ['-czf', archive, '-C', this.#root, 'package'],
    });
    return this.#package.archiveEntries
      ? this.#unsafeTarball(this.#package.archiveEntries)
      : await readFile(archive);
  }

  public versionMetadata(tarball: string, integrity: string): Record<string, Json> {
    return { ...this.#manifest, dist: { tarball, integrity } };
  }

  async #writeFiles(): Promise<void> {
    const pkg = join(this.#root, 'package');
    await mkdir(pkg, { recursive: true });
    await writeFile(join(pkg, 'package.json'), JSON.stringify(this.#manifest));
    for (const [name, content] of Object.entries(this.#package.files)) {
      await mkdir(join(pkg, name, '..'), { recursive: true });
      await writeFile(join(pkg, name), content);
    }
  }

  #unsafeTarball(entries: readonly TarEntry[]): Buffer {
    return gzipSync(
      Buffer.concat([
        tarEntry({ path: 'package/package.json', content: JSON.stringify(this.#manifest) }),
        ...Object.entries(this.#package.files).map(([path, content]) =>
          tarEntry({ path: `package/${path}`, content }),
        ),
        ...entries.map(tarEntry),
        Buffer.alloc(1024),
      ]),
    );
  }
}
