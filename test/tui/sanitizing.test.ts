import { expect, test } from 'vite-plus/test';
import { counterMachine, OAUTH_CODE } from '../../packages/tui/test/fixtures.ts';
import { AgentSession } from '../support/tui/agent-session.ts';
import { ENTER, PASTE_END, PASTE_START } from '../support/tui/keys.ts';
import { Session } from '../support/tui/session.ts';
import { UntrustedTurn } from '../support/tui/untrusted-turn.ts';

test('untrusted model, tool and run text cannot inject terminal control sequences', async () => {
  const sessionAgent = await AgentSession.open();
  const session = sessionAgent.session;
  const { terminal, run } = session;
  const turn = new UntrustedTurn(sessionAgent);
  await turn.requestTool();
  const screen = await turn.completeTool();
  expect(screen).toContain('safe');
  expect(screen).toContain('carriage');
  expect(screen).toContain('[image image/png');
  expect(screen).toContain('not rendered');
  const stream = terminal.writes.join('');
  for (const forbidden of [
    '\u001b]0;',
    '\u001b[9999;9999H',
    '\u009b',
    '\u001b_G',
    '\u001b]1337',
    '\u202E',
  ]) {
    expect(stream).not.toContain(forbidden);
  }
  expect(terminal.titles).toEqual([]);
  expect(JSON.stringify(run.runs.view(run.runId))).toContain('PWNED-TITLE');
  await session.finish();
  run.runs.stop(run.runId);
});

test.each(['e'.repeat(180), '\u001b'.repeat(3985), 'synthetic-server-credential'])(
  'auth errors never display raw credential-bearing payloads: %#',
  async (prefix) => {
    const session = await Session.open('counter', counterMachine);
    const { terminal, auth } = session;
    await session.screen.waitFor('"count": 0');
    auth.failure = new Error(prefix);
    session.screen.submit('/login oauth');
    await session.screen.waitFor('Complete login in your browser');
    terminal.sendInput(`${PASTE_START}${OAUTH_CODE}${PASTE_END}`);
    terminal.sendInput(ENTER);
    const failed = await session.screen.waitFor('Login failed.');
    // The code starts 185 characters into the message: truncating first would leave its prefix.
    expect(failed).not.toContain('synth');
    expect(terminal.writes.join('')).not.toContain('synth');
    await session.finish();
    session.run.runs.stop(session.run.runId);
  },
);
