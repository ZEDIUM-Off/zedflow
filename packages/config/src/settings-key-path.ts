import type { Json } from '@zf/core';
import { isObject } from './guards.ts';
import { isUnsafeKey } from './key-policy.ts';
import type { Settings } from './types.ts';

/** A validated dotted path into settings: every segment is nonempty and not a prototype key. */
export class SettingsKeyPath {
  readonly #parents: readonly string[];
  readonly #leaf: string;

  public constructor(dotted: string) {
    const segments = dotted.split('.');
    if (segments.some((part) => !part || isUnsafeKey(part))) {
      throw new Error('Invalid dotted settings path');
    }
    const leaf = segments.pop();
    if (!leaf) {
      throw new Error('Invalid dotted settings path');
    }
    this.#parents = segments;
    this.#leaf = leaf;
  }

  /**
   * Writes a copy of the value at the leaf, creating missing parents; an undefined value deletes
   * the leaf instead. Reports false when the path is absent so nothing needs writing.
   */
  public apply(settings: Settings, value: Json | undefined): boolean {
    const parent = this.#parent(settings, value !== undefined);
    if (!parent) {
      return false;
    }
    if (value !== undefined) {
      parent[this.#leaf] = structuredClone(value);
      return true;
    }
    return this.#delete(parent);
  }

  #delete(parent: Settings): boolean {
    if (!Object.hasOwn(parent, this.#leaf)) {
      return false;
    }
    delete parent[this.#leaf];
    return true;
  }

  #parent(settings: Settings, create: boolean): Settings | undefined {
    let target = settings;
    for (const segment of this.#parents) {
      const next = this.#descend(target, segment, create);
      if (!next) {
        return undefined;
      }
      target = next;
    }
    return target;
  }

  #descend(parent: Settings, segment: string, create: boolean): Settings | undefined {
    const child = parent[segment];
    if (child === undefined && !create) {
      return undefined;
    }
    if (child !== undefined && !isObject(child)) {
      throw new Error(`Cannot traverse non-object setting: ${segment}`);
    }
    const next = child ?? {};
    parent[segment] = next;
    return next;
  }
}
