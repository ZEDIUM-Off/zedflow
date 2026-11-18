import { relativePath } from './containment.ts';
import { safeDependencyManifest } from './dependency-manifest.ts';
import { installId, integrity } from './identifiers.ts';
import { object } from './json-values.ts';
import { parseSource } from './source-parser.ts';

const DEPENDENCY_ROOT = 'dependencies/node_modules';

/**
 * One locked source record. Constructing it proves the record is canonical and consistent, so a
 * receipt can only be checked against a record that has already passed.
 */
export class LockedSourceRecord {
  readonly #record: Record<string, unknown>;
  readonly #source: Record<string, unknown>;

  public constructor(id: string, record: unknown) {
    if (!installId(id) || !object(record) || !object(record['source'])) {
      throw new Error('Invalid locked source');
    }
    this.#record = record;
    this.#source = record['source'];
    if (!this.#valid()) {
      throw new Error('Invalid source integrity/dependency lock');
    }
  }

  /** Whether a receipt names the source, resolution and integrities this record locks. */
  public justifies(receipt: Record<string, unknown>): boolean {
    return (
      receipt['source'] === this.#source['normalized'] &&
      receipt['resolved'] === this.#record['resolved'] &&
      receipt['sourceIntegrity'] === this.#record['sourceIntegrity'] &&
      receipt['dependencyIntegrity'] === this.#record['dependencyIntegrity']
    );
  }

  #valid(): boolean {
    return this.#canonical() && this.#integrities() && this.#resolution() && this.#dependencies();
  }

  #canonical(): boolean {
    const normalized = this.#source['normalized'];
    if (typeof normalized !== 'string' || Object.keys(this.#source).length !== 4) {
      return false;
    }
    const parsed = parseSource(normalized);
    return (
      parsed.kind === this.#source['kind'] &&
      parsed.location === this.#source['location'] &&
      parsed.ref === this.#source['ref'] &&
      parsed.normalized === normalized
    );
  }

  #integrities(): boolean {
    return (
      typeof this.#record['resolved'] === 'string' &&
      integrity(this.#record['sourceIntegrity']) &&
      integrity(this.#record['dependencyIntegrity'])
    );
  }

  #resolution(): boolean {
    const resolved = this.#record['resolved'];
    if (this.#source['kind'] === 'git') {
      return typeof resolved === 'string' && /^[a-f0-9]{40,64}$/.test(resolved);
    }
    return this.#source['kind'] !== 'local' || resolved === this.#record['sourceIntegrity'];
  }

  #dependencies(): boolean {
    const lock = this.#record['dependencyLock'];
    return (
      safeDependencyManifest(this.#record['dependencyManifest']) &&
      (lock === null || typeof lock === 'string') &&
      (this.#record['dependencyPath'] === null) === (lock === null) &&
      this.#dependencyPath()
    );
  }

  #dependencyPath(): boolean {
    const path = this.#record['dependencyPath'];
    return (
      path === null ||
      (typeof path === 'string' &&
        relativePath(path) &&
        (path === DEPENDENCY_ROOT || path.startsWith(`${DEPENDENCY_ROOT}/`)))
    );
  }
}
