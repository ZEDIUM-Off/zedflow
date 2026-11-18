import type { Component, TUI } from '@earendil-works/pi-tui';
import type { RunUpdate } from '@zf/core';
import { AGENT_MACHINE_ID, isAgentActive, readAgentSnapshot } from './agent-view.ts';
import type { AgentSnapshot } from './agent-view.ts';
import { bold, dim, red } from './style.ts';
import { TextBlock } from './text-block.ts';

const RELOAD_PROGRESS: Readonly<Record<string, string>> = {
  waiting: 'reload: waiting for active work to finish',
  preparing: 'reload: preparing candidate',
};

/** Projects the latest run and local activity into the fixed screen chrome. */
export class TerminalChrome {
  readonly #tui: TUI;
  readonly #machineId: string;
  readonly #runId: string;
  readonly #header = new TextBlock({ style: bold, maxLines: 1 });
  readonly #status = new TextBlock({ style: dim, maxLines: 1 });
  readonly #activity = new TextBlock({ style: dim, maxLines: 1 });
  readonly #failure = new TextBlock({ style: red, maxLines: 3 });
  #snapshot: AgentSnapshot = readAgentSnapshot(null);
  #update: RunUpdate | undefined;
  #login = false;
  #reloading = false;

  public constructor(tui: TUI, machineId: string, runId: string) {
    this.#tui = tui;
    this.#machineId = machineId;
    this.#runId = runId;
  }

  public mount(content: Component, notices: Component, bottom: Component): void {
    for (const child of [
      this.#header,
      content,
      this.#status,
      this.#activity,
      this.#failure,
      notices,
      bottom,
    ]) {
      this.#tui.addChild(child);
    }
  }

  public update(update: RunUpdate, snapshot: AgentSnapshot): void {
    this.#update = update;
    this.#snapshot = snapshot;
    this.#refresh();
  }

  public login(active: boolean): void {
    this.#login = active;
    this.#refresh();
  }

  public reloading(active: boolean): void {
    // Starting a reload does not refresh until the run publishes its progress.
    this.#reloading = active;
    if (!active) {
      this.#refresh();
    }
  }

  #refresh(): void {
    this.#header.setText(
      `zedflow · ${this.#machineId} · run ${this.#runId.slice(0, 8)} · generation ${this.#update?.generation ?? '-'}`,
    );
    this.#status.setText(this.#statusParts().join(' · '));
    this.#activity.setText(this.#activityParts().join(' · '));
    this.#failure.setText(
      this.#machineId === AGENT_MACHINE_ID &&
        this.#snapshot.failure &&
        !isAgentActive(this.#snapshot)
        ? this.#snapshot.failure
        : '',
    );
    this.#tui.requestRender();
  }

  #statusParts(): string[] {
    const snapshot = this.#snapshot;
    return this.#machineId === AGENT_MACHINE_ID && snapshot.state
      ? [
          `state: ${snapshot.state}`,
          `model calls: ${snapshot.modelCalls ?? 0}`,
          `tool calls: ${snapshot.toolCalls ?? 0}`,
        ]
      : [];
  }

  #activityParts(): string[] {
    const activity: string[] = [];
    if (this.#reloading && this.#update?.reload && RELOAD_PROGRESS[this.#update.reload]) {
      activity.push(RELOAD_PROGRESS[this.#update.reload] ?? '');
    }
    if (this.#login) {
      activity.push('signing in');
    }
    return activity;
  }
}
