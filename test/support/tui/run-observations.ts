import type { RunFixture } from '../../../packages/tui/test/fixtures.ts';

export const inputs = (run: RunFixture): unknown[] =>
  run.runs.records(run.runId).filter((record) => record.kind === 'input');

export const stateOf = (run: RunFixture): unknown => {
  const view = run.runs.view(run.runId);
  return typeof view === 'object' && view !== null && !Array.isArray(view)
    ? view['value']
    : undefined;
};
