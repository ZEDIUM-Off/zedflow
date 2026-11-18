import type { ToolResultMessage } from '@earendil-works/pi-ai';
// Ephemeral script-facing values never become serializable run-history fields.
class ScriptResultStore {
  readonly #values = new WeakMap<ToolResultMessage, unknown>();
  public set(result: ToolResultMessage, value: unknown): void {
    this.#values.set(result, value);
  }
  public get(result: ToolResultMessage): unknown {
    return this.#values.get(result);
  }
}
export const scriptResults = new ScriptResultStore();
