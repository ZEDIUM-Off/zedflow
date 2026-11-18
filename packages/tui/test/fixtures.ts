import type { AnyStateMachine } from 'xstate';
import counter from '../../machines/counter/machine.ts';
import type { AgentControls } from './agent-controls.ts';
import { agentMachine } from './agent-machine.ts';

// Re-exported so tests resolve Pi TUI through this package, not the workspace root.
export { stripTerminalSequences, visibleWidth } from '@earendil-works/pi-tui';
export { AgentControls } from './agent-controls.ts';
export { agentMachine } from './agent-machine.ts';
export { createAuthFixture } from './auth-fixture.ts';
export type { AuthFixture } from './auth-fixture.ts';
export { gate } from './gate.ts';
export type { Gate } from './gate.ts';
export { assistant, textEvent } from './messages.ts';
export { createRun } from './run-fixture.ts';
export type { RunFixture } from './run-fixture.ts';

export const SECRET = 'synthetic-secret-key-4242';
export const OAUTH_CODE = 'synthetic-oauth-code-9876';
export const counterMachine = (): AnyStateMachine => counter;

/** Same native identity, new version: a reload candidate the coordinator must refuse. */
export const versioned =
  (controls: AgentControls): ((load: number) => AnyStateMachine) =>
  (load) =>
    agentMachine(controls, load > 1 ? '2' : '1');
