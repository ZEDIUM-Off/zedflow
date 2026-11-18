import type { RunService } from '@zf/core';
import { expect, test } from 'vite-plus/test';
import { TerminalUI } from '../../packages/tui/src/index.ts';
import {
  counterMachine,
  createAuthFixture,
  createRun,
  gate,
  SECRET,
} from '../../packages/tui/test/fixtures.ts';
import { VirtualTerminal } from '../../packages/tui/test/virtual-terminal.ts';
import { AgentSession } from '../support/tui/agent-session.ts';
import { CTRL_C, CTRL_D } from '../support/tui/keys.ts';
import { inputs, stateOf } from '../support/tui/run-observations.ts';
import { Session } from '../support/tui/session.ts';

test('a generic native machine is presented and driven through JSON events from run subscriptions', async () => {
  const session = await Session.open('counter', counterMachine);
  const { terminal, run } = session;
  await session.screen.waitFor('"count": 0');
  session.screen.submit('{"type":"INCREMENT"}');
  await session.screen.waitFor('"count": 1');
  session.screen.submit('not json');
  await session.screen.waitFor('Input is not JSON');
  session.screen.submit('{"count":1}');
  await session.screen.waitFor('needs a string "type"');
  expect(inputs(run)).toHaveLength(1);
  // An event sent by another actor of the process arrives through the subscription alone.
  run.runs.send(run.runId, { type: 'INCREMENT' });
  await session.screen.waitFor('"count": 2');
  session.screen.submit('/reset');
  await session.screen.waitFor('"count": 0');
  session.screen.submit('/help');
  const help = await session.screen.waitFor('/quit');
  for (const command of ['/help', '/reload', '/login', '/logout', '/auth', '/reset']) {
    expect(help).toContain(command);
  }
  session.screen.submit('/nonsense');
  await session.screen.waitFor('Unknown command /nonsense');

  const writes = terminal.writes.length;
  const stopped = session.ui.stop();
  expect(session.ui.stop()).toBe(stopped);
  await stopped;
  await session.done;
  expect(session.ui.start()).toBe(session.done);
  expect(terminal.starts).toBe(1);
  expect(terminal.stops).toBe(1);
  expect(terminal.drains).toBe(1);
  expect(terminal.running).toBe(false);
  // Detached from the run and the terminal: later events render nothing.
  const afterStop = terminal.writes.length;
  run.runs.send(run.runId, { type: 'INCREMENT' });
  await new Promise((resolve) => setTimeout(resolve, 60));
  expect(terminal.writes.length).toBe(afterStop);
  expect(afterStop).toBeGreaterThanOrEqual(writes);
  run.runs.stop(run.runId);
});

test('/quit, Ctrl+D, idle Ctrl+C and an externally stopped run each close the UI and resolve start', async () => {
  for (const close of [
    (s: Session): void => s.screen.submit('/quit'),
    (s: Session): void => s.terminal.sendInput(CTRL_D),
    (s: Session): void => s.terminal.sendInput(CTRL_C),
    (s: Session): void => s.run.runs.stop(s.run.runId),
  ]) {
    const session = await Session.open('counter', counterMachine);
    await session.screen.waitFor('"count": 0');
    close(session);
    await session.done;
    expect(session.terminal.running).toBe(false);
    expect(session.terminal.stops).toBe(1);
    if (stateOf(session.run) !== undefined) {
      session.run.runs.stop(session.run.runId);
    }
  }
  const session = await Session.open('counter', counterMachine);
  await session.screen.waitFor('"count": 0');
  session.screen.type('draft');
  session.terminal.sendInput(CTRL_D);
  await session.terminal.screen();
  expect(session.terminal.running).toBe(true);
  await session.finish();
  session.run.runs.stop(session.run.runId);
});

test('stopping the UI cancels a pending login and a pending reload before the run is stopped', async () => {
  const session = await Session.open('counter', counterMachine);
  await session.screen.waitFor('"count": 0');
  session.screen.submit('/login api-key');
  await session.screen.waitFor('Enter OpenAI API key');
  session.screen.type(SECRET);
  await session.ui.stop();
  await session.done;
  expect(await session.auth.credentials.read('openai')).toBeUndefined();
  expect(session.terminal.writes.join('')).not.toContain(SECRET);
  session.run.runs.stop(session.run.runId);

  const slow = gate<undefined>();
  const preparing = await Session.open('counter', counterMachine, {
    slowLoad: slow.promise.then(() => {}),
  });
  await preparing.screen.waitFor('"count": 0');
  preparing.screen.submit('/reload');
  await preparing.screen.waitFor('reload: preparing candidate');
  await preparing.ui.stop();
  await preparing.done;
  // The cancelled reload released the run; stopping it must not report it as unavailable.
  expect(() => preparing.run.runs.stop(preparing.run.runId)).not.toThrow();
  slow.resolve(undefined);
  await new Promise((resolve) => setTimeout(resolve, 20));
  expect(
    preparing.run.runs.records(preparing.run.runId).some((record) => record.kind === 'replacement'),
  ).toBe(false);
});

test('closing while a reload waits for active work cancels it and leaves the effect running', async () => {
  const sessionAgent = await AgentSession.open();
  const session = sessionAgent.session;
  const { run } = session;
  const controls = sessionAgent.controls;
  await session.screen.waitFor('state: ready');
  session.screen.submit('long work');
  await session.screen.waitFor('state: thinking');
  session.screen.submit('/reload');
  await session.screen.waitFor('reload: waiting');
  await session.ui.stop();
  await session.done;
  expect(controls.aborted).toBe(0);
  expect(stateOf(run)).toBe('thinking');
  expect(() => run.runs.stop(run.runId)).not.toThrow();
});

test('a start that fails after the terminal opened restores it once and a later stop does not repeat that', async () => {
  const run = await createRun(counterMachine);
  const failing: RunService = {
    launch: (path, origin) => run.runs.launch(path, origin),
    send: (runId, event) => run.runs.send(runId, event),
    view: (runId) => run.runs.view(runId),
    records: (runId) => run.runs.records(runId),
    reload: (runId, options) => run.runs.reload(runId, options),
    subscribe: () => {
      throw new Error('subscribe failed');
    },
    stop: (runId) => run.runs.stop(runId),
  };
  const terminal = new VirtualTerminal(60, 12);
  const ui = new TerminalUI({
    runs: failing,
    runId: run.runId,
    machineId: 'counter',
    models: createAuthFixture().models,
    terminal,
  });
  expect(() => ui.start()).toThrow('subscribe failed');
  expect(terminal.stops).toBe(1);
  await ui.stop();
  await ui.stop();
  expect(terminal.stops).toBe(1);
  expect(terminal.drains).toBe(0);
  expect(terminal.running).toBe(false);
  run.runs.stop(run.runId);
});
