import { resolve } from 'node:path';
import type { Source } from './types.ts';

/**
 * The text after `git:` split into location and ref. The spec becomes a Source only after the ref
 * is safe and the location is a credential-free local path or HTTP(S) URL, so no SSH or external
 * transport helper can reach Git.
 */
export class GitSourceSpec {
  readonly #location: string;
  readonly #ref: string;

  public constructor(spec: string) {
    const separator = spec.lastIndexOf('#');
    this.#location = separator < 0 ? spec : spec.slice(0, separator);
    this.#ref = separator < 0 ? 'HEAD' : spec.slice(separator + 1);
  }

  public source(): Source {
    const ref = this.#ref;
    if (!this.#location || !ref || ref.startsWith('-') || /[\s\0]/.test(ref)) {
      throw new Error('Invalid Git source/ref');
    }
    const location = this.#canonicalLocation();
    return { kind: 'git', location, ref, normalized: `git:${location}#${ref}` };
  }

  #canonicalLocation(): string {
    if (this.#location.startsWith('file:')) {
      return this.#localFilePath();
    }
    if (/^https?:\/\//.test(this.#location)) {
      this.#rejectCredentials();
      return this.#location;
    }
    if (this.#location.includes(':') || this.#location.startsWith('-')) {
      throw new Error(
        'Git transport must be a local path or HTTP(S); SSH/external helpers are disabled',
      );
    }
    return resolve(this.#location);
  }

  #localFilePath(): string {
    const url = new URL(this.#location);
    if ((url.hostname && url.hostname !== 'localhost') || url.username || url.password) {
      throw new Error('Only credential-free local file Git URLs are allowed');
    }
    return decodeURIComponent(url.pathname);
  }

  #rejectCredentials(): void {
    const url = new URL(this.#location);
    if (url.username || url.password) {
      throw new Error('Credentials are not allowed in source URLs');
    }
  }
}
