import type { AuthInteraction } from '@earendil-works/pi-ai';
import type { TerminalUIOptions } from '@zf/tui';
import type { ApplicationOptions } from './application.ts';
export interface CommandOptions {
  readonly provider?: ApplicationOptions['provider'];
  readonly authContext?: ApplicationOptions['authContext'];
  readonly interaction?: AuthInteraction;
  readonly signal?: AbortSignal;
  readonly terminal?: TerminalUIOptions['terminal'];
  readonly write?: (text: string) => void;
  readonly error?: (text: string) => void;
}
