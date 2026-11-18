import { randomUUID } from 'node:crypto';
import { open, rename, rm } from 'node:fs/promises';
import { SettingsLock } from './settings-lock.ts';
import type { Settings } from './types.ts';

/** Replaces a settings file atomically while holding its lock; `end` always cleans up. */
export class SettingsTransaction {
  readonly #path: string;
  readonly #temporary: string;
  readonly #lock: SettingsLock;

  public constructor(path: string) {
    this.#path = path;
    this.#temporary = `${path}.${randomUUID()}.tmp`;
    this.#lock = new SettingsLock(path);
  }

  public async begin(): Promise<void> {
    await this.#lock.acquire();
  }

  public async commit(settings: Settings): Promise<void> {
    const file = await open(this.#temporary, 'wx', 0o600);
    try {
      await file.writeFile(`${JSON.stringify(settings, null, 2)}\n`);
      await file.sync();
    } finally {
      await file.close();
    }
    this.#lock.assertHeld();
    await rename(this.#temporary, this.#path);
  }

  public async end(): Promise<void> {
    await rm(this.#temporary, { force: true });
    await this.#lock.release();
  }
}
