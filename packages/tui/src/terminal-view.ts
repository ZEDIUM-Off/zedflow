import type { Component, TUI } from '@earendil-works/pi-tui';
import type { AssistantMessageEvent, ToolResultMessage } from '@earendil-works/pi-ai';
import type { RunUpdate } from '@zf/core';
import { AGENT_MACHINE_ID } from './agent-view.ts';
import type { AgentSnapshot } from './agent-view.ts';
import { StateView } from './state-view.ts';
import { TranscriptView } from './transcript-view.ts';
import { TerminalChrome } from './terminal-chrome.ts';

/** Owns the generic/agent presentation choice and in-flight observer content. */
export class TerminalView {
  public readonly agent: boolean;
  public readonly chrome: TerminalChrome;
  readonly #tui: TUI;
  readonly #state = new StateView();
  readonly #transcript = new TranscriptView();

  public constructor(tui: TUI, machineId: string, runId: string) {
    this.#tui = tui;
    this.agent = machineId === AGENT_MACHINE_ID;
    this.chrome = new TerminalChrome(tui, machineId, runId);
  }

  public mount(notices: Component, bottom: Component): void {
    this.chrome.mount(this.agent ? this.#transcript : this.#state, notices, bottom);
  }

  public update(update: RunUpdate, snapshot: AgentSnapshot): void {
    if (this.agent) {
      this.#transcript.setMessages(snapshot.messages);
      if (snapshot.state !== 'thinking') {
        this.#transcript.setStreaming(undefined);
      }
      if (snapshot.state !== 'tool') {
        this.#transcript.setToolProgress(undefined);
      }
    } else {
      this.#state.setView(update.view);
    }
    this.chrome.update(update, snapshot);
  }

  public modelEvent(event: AssistantMessageEvent): void {
    this.#transcript.setStreaming(
      event.type === 'done' || event.type === 'error' ? undefined : event.partial,
    );
    this.#tui.requestRender();
  }

  public toolUpdate(result: ToolResultMessage): void {
    this.#transcript.setToolProgress(result);
    this.#tui.requestRender();
  }

  public reset(): void {
    this.#transcript.setStreaming(undefined);
  }
}
