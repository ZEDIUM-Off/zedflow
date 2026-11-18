import type { Transport } from '@statelyai/sdk/inspect';

export class InspectorTransport implements Transport {
  public readonly ready = true;
  readonly #sent: Parameters<Transport['send']>[0][] = [];
  #receive: Parameters<Transport['onMessage']>[0] | undefined;
  #destroyed = false;

  public get sent(): Parameters<Transport['send']>[0][] {
    return this.#sent;
  }
  public get receive(): Parameters<Transport['onMessage']>[0] | undefined {
    return this.#receive;
  }
  public get destroyed(): boolean {
    return this.#destroyed;
  }

  public send(message: Parameters<Transport['send']>[0]): void {
    this.#sent.push(message);
  }

  public onMessage(handler: Parameters<Transport['onMessage']>[0]): () => void {
    this.#receive = handler;
    return () => {
      this.#receive = undefined;
    };
  }

  public onReady(): () => void {
    return () => {};
  }

  public destroy(): void {
    this.#destroyed = true;
  }
}
