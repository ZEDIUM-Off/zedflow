import { expect, test } from 'vite-plus/test';
import { assistant } from '../../packages/tui/test/fixtures.ts';
import { AgentSession } from '../support/tui/agent-session.ts';

test('/reload waits for active native work, then applies; an incompatible candidate is refused', async () => {
  const sessionAgent = await AgentSession.open();
  const session = sessionAgent.session;
  const { run } = session;
  const controls = sessionAgent.controls;
  await session.screen.waitFor('state: ready');
  session.screen.submit('work');
  await session.screen.waitFor('state: thinking');
  session.screen.submit('/reload');
  const waiting = await session.screen.waitFor('reload: waiting for active work to finish');
  expect(waiting).toContain('generation 1');
  expect(run.loads()).toBe(1);
  session.screen.submit('/reload');
  await session.screen.waitFor('A reload is already pending.');
  controls.finishModel(assistant([{ type: 'text', text: 'native turn finished' }]));
  const applied = await session.screen.waitFor('Reload applied.');
  expect(applied).toContain('generation 2');
  expect(applied).toContain('native turn finished');
  expect(controls.aborted).toBe(0);
  await session.finish();
  run.runs.stop(run.runId);

  // The second load reaches a different native version: refused, the old generation still serves input.
  const refusedAgent = await AgentSession.open({ refuse: true });
  const refused = refusedAgent.session;
  await refused.screen.waitFor('state: ready');
  refused.screen.submit('/reload');
  const message = await refused.screen.waitFor('Reload refused:');
  expect(message).toContain('identity/version');
  refused.screen.submit('still works');
  await refused.screen.waitFor('state: thinking');
  await refused.finish();
  refused.run.runs.stop(refused.run.runId);
});
