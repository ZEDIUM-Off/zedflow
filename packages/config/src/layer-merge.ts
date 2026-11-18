import type { Json } from '@zf/core';
import { isObject } from './guards.ts';
import type { ProvenanceLedger } from './provenance-ledger.ts';
import type { ConfigurationLayer, Settings } from './types.ts';

/** Merges one layer into a settings tree: arrays and scalars replace, objects merge. */
export class LayerMerge {
  readonly #layer: ConfigurationLayer;
  readonly #ledger: ProvenanceLedger;

  public constructor(layer: ConfigurationLayer, ledger: ProvenanceLedger) {
    this.#layer = layer;
    this.#ledger = ledger;
  }

  public into(target: Settings, source: Settings, prefix: string): void {
    for (const [key, value] of Object.entries(source)) {
      const path = prefix ? `${prefix}.${key}` : key;
      if (isObject(value)) {
        this.#mergeObject(target, key, value, path);
      } else {
        this.#replaceLeaf(target, key, value, path);
      }
    }
  }

  #mergeObject(target: Settings, key: string, value: Settings, path: string): void {
    const previous = target[key];
    if (isObject(previous)) {
      this.#ledger.forget(path);
    } else {
      this.#ledger.forgetSubtree(path);
    }
    const child = isObject(previous) ? previous : {};
    target[key] = child;
    this.into(child, value, path);
    if (Object.keys(child).length === 0) {
      this.#ledger.record(path, this.#layer);
    }
  }

  #replaceLeaf(target: Settings, key: string, value: Json, path: string): void {
    this.#ledger.forgetSubtree(path);
    target[key] = structuredClone(value);
    this.#ledger.record(path, this.#layer);
  }
}
