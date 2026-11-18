// Development composition only; the caller owns the injected transport.
import { createInspector } from '@statelyai/sdk/inspect';
import type { Inspector, Transport } from '@statelyai/sdk/inspect';
import { XStateEngine } from '@zf/xstate';
import type { Engine } from '@zf/core';

export class DevelopmentInspector {
  public readonly inspector: Inspector;
  public readonly engine: Engine;

  public constructor(transport: Transport) {
    this.inspector = createInspector({ transport, launch: 'none', bidirectional: true });
    this.engine = new XStateEngine({ inspect: this.inspector.inspect });
  }

  public dispose(): void {
    this.inspector.destroy();
  }
}
