// Adapted from Pi 1.5.2 core/auth-storage.ts: native CredentialStore and locked refresh.
// Copyright (c) 2025 Mario Zechner. MIT notice: ../UPSTREAM-LICENSE.
import type {
  AuthOperationOptions,
  Credential,
  CredentialInfo,
  CredentialStore,
} from '@earendil-works/pi-ai';
import { CredentialFile } from './credential-file.ts';
import { decodeCredential } from './credential-codec.ts';

/** Pi owns authentication and token refresh; this class only persists its credentials. */
export class JsonCredentialStore implements CredentialStore {
  readonly #file: CredentialFile;

  public constructor(path: string) {
    this.#file = new CredentialFile(path);
  }

  public async read(
    providerId: string,
    options?: AuthOperationOptions,
  ): Promise<Credential | undefined> {
    options?.signal?.throwIfAborted();
    const value = (await this.#file.read()).get(providerId);
    options?.signal?.throwIfAborted();
    return value;
  }

  public async list(options?: AuthOperationOptions): Promise<readonly CredentialInfo[]> {
    options?.signal?.throwIfAborted();
    const data = await this.#file.read();
    options?.signal?.throwIfAborted();
    return [...data].map(([providerId, value]) => ({ providerId, type: value.type }));
  }

  public async modify(
    providerId: string,
    fn: (current: Credential | undefined) => Promise<Credential | undefined>,
    options?: AuthOperationOptions,
  ): Promise<Credential | undefined> {
    if (!providerId || Buffer.from(providerId).some((byte) => byte < 32)) {
      throw new Error('Invalid credential provider');
    }
    return this.#file.transaction(async (data) => {
      const previous = data.get(providerId);
      const next = await fn(structuredClone(previous));
      if (next === undefined) {
        return { result: previous, changed: false };
      }
      const checked = decodeCredential(next);
      data.set(providerId, checked);
      return { result: checked, changed: true };
    }, options);
  }

  public async delete(providerId: string, options?: AuthOperationOptions): Promise<void> {
    await this.#file.transaction(
      async (data) => ({ result: undefined, changed: data.delete(providerId) }),
      options,
    );
  }
}
