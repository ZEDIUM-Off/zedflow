import type { ConfigurationLayer } from './types.ts';

/** Records the layer that last wrote each leaf path; a write invalidates what it replaces. */
export class ProvenanceLedger {
  readonly #layers: Record<string, ConfigurationLayer> = {};

  public record(path: string, layer: ConfigurationLayer): void {
    this.#layers[path] = layer;
  }

  public forget(path: string): void {
    delete this.#layers[path];
  }

  public forgetSubtree(path: string): void {
    for (const leaf of Object.keys(this.#layers)) {
      if (leaf === path || leaf.startsWith(`${path}.`)) {
        delete this.#layers[leaf];
      }
    }
  }

  public layers(): Record<string, ConfigurationLayer> {
    return this.#layers;
  }
}
