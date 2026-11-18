// Adapted from Pi1.5.2 cli/auth-terminal.ts, cli/auth-login.ts and utils/terminal-question.ts.
// Copyright Mario Zechner. MIT notice: ../UPSTREAM-LICENSE.
import { randomUUID } from 'node:crypto';
import {
  constants,
  fsyncSync,
  linkSync,
  unlinkSync,
  closeSync,
  fstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  writeFileSync,
} from 'node:fs';
import { join } from 'node:path';
export class DeviceIdentity {
  readonly #directory: string;
  public constructor(directory: string) {
    this.#directory = directory;
  }
  public get(): string {
    mkdirSync(this.#directory, { recursive: true, mode: 0o700 });
    const path = join(this.#directory, 'device-id');
    try {
      return this.#read(path);
    } catch (error) {
      if (!(error instanceof Error) || !('code' in error) || error.code !== 'ENOENT') {
        throw error;
      }
    }
    return this.#publish(path);
  }
  #publish(path: string): string {
    const temporary = join(this.#directory, `.device-id.${randomUUID()}.tmp`);
    const fd = openSync(
      temporary,
      constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW,
      0o600,
    );
    try {
      this.#write(fd);
      try {
        linkSync(temporary, path);
      } catch (error) {
        if (!(error instanceof Error) || !('code' in error) || error.code !== 'EEXIST') {
          throw new Error('Cannot create installation identity');
        }
      }
      return this.#read(path);
    } finally {
      unlinkSync(temporary);
    }
  }
  #write(fd: number): void {
    try {
      writeFileSync(fd, `${randomUUID()}\n`);
      fsyncSync(fd);
    } finally {
      closeSync(fd);
    }
  }
  #read(path: string): string {
    const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
    try {
      const stat = fstatSync(fd);
      if (!stat.isFile() || stat.size > 128) {
        throw new Error('Invalid installation identity');
      }
      const id = readFileSync(fd, 'utf8').trim();
      if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(id)) {
        throw new Error('Invalid installation identity');
      }
      return id;
    } finally {
      closeSync(fd);
    }
  }
}
