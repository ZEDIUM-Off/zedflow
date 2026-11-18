import { expect, test } from 'vite-plus/test';
import { assistant, textEvent } from '../../packages/tui/test/fixtures.ts';
import { AgentSession } from '../support/tui/agent-session.ts';
import { ESCAPE } from '../support/tui/keys.ts';
import { inputs } from '../support/tui/run-observations.ts';

test('the agent UI streams model and tool updates, takes the final transcript from run state and cancels natively', async () => {
  const sessionAgent = await AgentSession.open();
  const session = sessionAgent.session;
  const { terminal, run } = session;
  const controls = sessionAgent.controls;
  await session.screen.waitFor('state: ready');

  // Cancellation requested while idle is harmless: no event is sent.
  terminal.sendInput(ESCAPE);
  await terminal.screen();
  expect(inputs(run)).toHaveLength(0);
  session.screen.submit('   ');
  expect(inputs(run)).toHaveLength(0);

  await sessionAgent.beginPrompt('hello agent');
  expect(await session.screen.waitFor('> hello agent')).toContain('model calls: 1');
  controls.observeModel(textEvent('streaming partial'));
  await session.screen.waitFor('streaming partial');
  session.screen.submit('second prompt while busy');
  await session.screen.waitFor('The agent is busy');
  expect(inputs(run)).toHaveLength(1);

  const running = await sessionAgent.startLookup();
  expect(running).toContain('lookup(');
  const settled = await sessionAgent.finishLookup();
  expect(settled).toContain('final answer');
  expect(settled).toContain('tool final text');
  expect(settled).not.toContain('streaming partial');
  expect(settled).not.toContain('tool progress text');
  expect(settled).toContain('tool calls: 1');

  await sessionAgent.beginPrompt('cancel me');
  terminal.sendInput(ESCAPE);
  const cancelled = await session.screen.waitFor('state: cancelled');
  expect(cancelled).toContain('Cancelled; in-flight tool effects may be incomplete');
  expect(controls.aborted).toBe(1);
  // A new prompt after cancellation preserves the earlier conversation.
  await sessionAgent.beginPrompt('again');
  expect(controls.requests.at(-1)?.map((message) => message.role)).toEqual([
    'user',
    'assistant',
    'toolResult',
    'assistant',
    'user',
    'user',
  ]);
  controls.finishModel(assistant([{ type: 'text', text: 'second answer' }]));
  await session.screen.waitFor('second answer');
  session.screen.submit('/reset');
  await session.screen.waitFor('Reset sent.');
  const reset = await session.screen.screen();
  expect(reset).not.toContain('second answer');
  expect(reset).toContain('state: ready');
  await session.finish();
  run.runs.stop(run.runId);
});
