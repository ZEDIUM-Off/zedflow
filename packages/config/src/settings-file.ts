import { lstat, mkdir, readFile } from 'node:fs/promises';
import { dirname } from 'node:path';
import type { Json } from '@zf/core';
import { isMissingFile } from './guards.ts';
import type { SettingsKeyPath } from './settings-key-path.ts';
import { SettingsTransaction } from './settings-transaction.ts';
import { validateSettings } from './settings-validation.ts';
import type { Settings } from './types.ts';

/** One settings file: validated reads and lock-protected atomic read-modify-write changes. */
export class SettingsFile {
  readonly #path: string;

  public constructor(path: string) {
    this.#path = path;
  }

  public async read(): Promise<Settings> {
    try {
      return validateSettings(await this.#parse());
    } catch (error) {
      if (isMissingFile(error)) {
        return {};
      }
      throw new Error(`Invalid settings in ${this.#path}: ${this.#reason(error)}`);
    }
  }

  public async set(key: SettingsKeyPath, value: Json): Promise<void> {
    await this.#change(key, value);
  }

  public async unset(key: SettingsKeyPath): Promise<void> {
    await this.#change(key, undefined);
  }

  async #change(key: SettingsKeyPath, value: Json | undefined): Promise<void> {
    if (value === undefined && !(await this.#exists())) {
      return;
    }
    await mkdir(dirname(this.#path), { recursive: true });
    const transaction = new SettingsTransaction(this.#path);
    await transaction.begin();
    try {
      const settings = await this.read();
      if (key.apply(settings, value)) {
        validateSettings(settings);
        await transaction.commit(settings);
      }
    } finally {
      await transaction.end();
    }
  }

  async #parse(): Promise<unknown> {
    if (!(await lstat(this.#path)).isFile()) {
      throw new Error(`Settings must be a regular file: ${this.#path}`);
    }
    const content = (await readFile(this.#path, 'utf8')).replace(/^\uFEFF/, '');
    return content.trim() ? JSON.parse(content) : {};
  }

  async #exists(): Promise<boolean> {
    try {
      await lstat(this.#path);
      return true;
    } catch (error) {
      if (isMissingFile(error)) {
        return false;
      }
      throw error;
    }
  }

  #reason(error: unknown): string {
    if (error instanceof SyntaxError) {
      return 'Invalid settings JSON';
    }
    return error instanceof Error ? error.message : 'Unable to read settings';
  }
}
