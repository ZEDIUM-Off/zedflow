import { expect, test } from 'vite-plus/test';
import { assistant, visibleWidth } from '../../packages/tui/test/fixtures.ts';
import { AgentSession } from '../support/tui/agent-session.ts';
import { WidthComponents } from '../support/tui/width-components.ts';

test('rendering stays within every width for multibyte, wide and long unbroken text', async () => {
  const wide =
    '日本語のテキスト🙂👩‍👩‍👧‍👦 e\u0301 ' + 'x'.repeat(120) + ' https://example.test/' + 'a'.repeat(200);
  const components = new WidthComponents(wide);
  for (let width = 1; width <= 40; width += 1) {
    for (const component of components.views) {
      for (const row of component.render(width)) {
        expect(visibleWidth(row)).toBeLessThanOrEqual(width);
      }
    }
  }
  components.close();

  const sessionAgent = await AgentSession.open({ columns: 60, rows: 12 });
  const session = sessionAgent.session;
  const { terminal } = session;
  const controls = sessionAgent.controls;
  session.screen.submit(wide);
  controls.finishModel(assistant([{ type: 'text', text: wide }]));
  await session.screen.waitFor('state: ready');
  for (const [columns, rows] of [
    [20, 8],
    [5, 5],
    [3, 4],
    [90, 20],
  ] as const) {
    terminal.resize(columns, rows);
    const screen = await terminal.screen();
    expect(screen.length).toBe(rows);
    expect(terminal.running).toBe(true);
  }
  expect(await session.screen.screen()).toContain('state: ready');
  await session.finish();
  session.run.runs.stop(session.run.runId);
});
