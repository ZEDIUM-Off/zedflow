import { getCurrentTools } from '@earendil-works/pi-ai';
import { expect, test } from 'vite-plus/test';
import { fixture } from '../support/cli/fixture.ts';
import { ReloadFixture } from '../support/cli/reload-fixture.ts';
import { guardNetwork } from '../support/cli/network-guard.ts';

guardNetwork();

test('full resource reload waits for a native model effect and swaps config/tools/skills together', async () => {
  const f = await fixture();
  const reload = new ReloadFixture(f);
  const app = reload.app;
  const id = await reload.start();
  await reload.prepareCandidate();
  let applied = false;
  const pending = app.runs
    .reload(id, { wait: true, signal: new AbortController().signal })
    .then(() => {
      applied = true;
    });
  await Promise.resolve();
  expect(applied).toBe(false);
  expect(reload.native.contexts).toHaveLength(1);
  expect(reload.native.aborted).toBe(false);
  reload.native.finish();
  await pending;
  expect(reload.native.aborted).toBe(false);
  expect(app.runs.view(id)).toMatchObject({ value: 'ready', context: { modelCalls: 1 } });
  app.runs.send(id, { type: 'PROMPT', text: 'second' });
  await expect.poll(() => app.runs.view(id)).toMatchObject({ value: 'ready' });
  expect(reload.native.contexts).toHaveLength(2);
  const first = reload.native.contexts[0];
  const second = reload.native.contexts[1];
  if (!first || !second) {
    throw new Error('Missing native requests');
  }
  expect(getCurrentTools(first.messages).map((tool) => tool.name)).toContain('read');
  expect(getCurrentTools(second.messages).map((tool) => tool.name)).toEqual(['codemode']);
  expect(JSON.stringify(first)).toContain('before reload');
  expect(JSON.stringify(second)).toContain('after reload');
  expect(JSON.stringify(second)).toContain('answer 1');
  expect(app.runs.records(id).filter((record) => record.kind === 'replacement')).toHaveLength(1);
  app.runs.stop(id);
});
