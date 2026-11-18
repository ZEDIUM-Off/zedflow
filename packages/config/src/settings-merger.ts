import { LayerMerge } from './layer-merge.ts';
import { ProvenanceLedger } from './provenance-ledger.ts';
import type { ConfigurationLayer, ResolvedConfiguration, Settings } from './types.ts';

/** Accumulates layers in application order into one settings tree and its leaf provenance. */
export class SettingsMerger {
  readonly #settings: Settings = {};
  readonly #ledger = new ProvenanceLedger();

  public apply(value: Settings, layer: ConfigurationLayer): void {
    new LayerMerge(layer, this.#ledger).into(this.#settings, value, '');
  }

  public result(): ResolvedConfiguration {
    return { settings: this.#settings, provenance: this.#ledger.layers() };
  }
}
