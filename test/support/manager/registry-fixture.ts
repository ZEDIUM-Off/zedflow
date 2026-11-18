import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import type { ServerResponse } from 'node:http';
import type { Json } from '@zf/core';
import { PackageArchive } from './package-archive.ts';
import type { PublishedPackage } from './package-archive.ts';

export class RegistryFixture {
  readonly #directory: string;
  readonly #packages = new Map<string, Record<string, Json>>();
  readonly #tarballs = new Map<string, Buffer>();
  readonly #requests: string[] = [];
  readonly #server = createServer((request, response) => {
    const path = decodeURIComponent(request.url ?? '/');
    this.#requests.push(path);
    const tarball = this.#tarballs.get(path);
    if (tarball) {
      response.setHeader('content-type', 'application/octet-stream');
      response.end(tarball);
      return;
    }
    this.#serveMetadata(path, response);
  });

  public constructor(directory: string) {
    this.#directory = directory;
  }

  #serveMetadata(path: string, response: ServerResponse): void {
    const metadata = this.#packages.get(path.slice(1));
    if (!metadata) {
      response.writeHead(404);
      response.end('{}');
      return;
    }
    response.setHeader('content-type', 'application/json');
    response.end(JSON.stringify(metadata));
  }
  public url(): string {
    const address = this.#server.address();
    if (!address || typeof address === 'string') {
      throw new Error('Fixture registry not listening');
    }
    return `http://127.0.0.1:${address.port}/`;
  }
  public requests(): readonly string[] {
    return this.#requests;
  }
  public corruptTarballs(): void {
    for (const path of this.#tarballs.keys()) {
      this.#tarballs.set(path, Buffer.from('fixture archive integrity mismatch'));
    }
  }
  public async start(): Promise<void> {
    await new Promise<void>((accept, reject) => {
      this.#server.once('error', reject);
      this.#server.listen(0, '127.0.0.1', accept);
    });
  }
  public async close(): Promise<void> {
    this.#server.closeAllConnections();
    await new Promise<void>((accept, reject) =>
      this.#server.close((error) => (error ? reject(error) : accept())),
    );
  }
  public async publish(input: PublishedPackage): Promise<void> {
    const archive = new PackageArchive(this.#directory, input);
    const tarball = await archive.build();
    const path = `/${input.name}/-/${input.version}.tgz`;
    this.#tarballs.set(path, tarball);
    const versions = this.#packages.get(input.name)?.['versions'];
    this.#packages.set(input.name, {
      name: input.name,
      'dist-tags': { latest: input.version },
      versions: {
        ...(typeof versions === 'object' && versions !== null && !Array.isArray(versions)
          ? versions
          : {}),
        [input.version]: archive.versionMetadata(
          `${this.url().slice(0, -1)}${path}`,
          `sha512-${createHash('sha512').update(tarball).digest('base64')}`,
        ),
      },
    });
  }
}
