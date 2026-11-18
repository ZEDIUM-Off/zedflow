import { RunCoordinator } from '@zf/run';
import { JsonlRunStore } from '@zf/storage';
import { XStateEngine } from '@zf/xstate';
import { mkdir, mkdtemp } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import type { AnyStateMachine } from 'xstate';
import { RunLoader } from './run-loader.ts';

export interface RunFixture {
  readonly runs: RunCoordinator;
  readonly runId: string;
  readonly loads: () => number;
}

/** Real coordinator and native XState engine; the loader serves `machine()` on each load. */
export const createRun = async (
  machine: (load: number) => AnyStateMachine,
  options: { readonly slowLoad?: Promise<void> } = {},
): Promise<RunFixture> => {
  const root = resolve('.artifacts/cli-goal/tui-tests');
  await mkdir(root, { recursive: true });
  const directory = await mkdtemp(join(root, 'run-'));
  const loader = new RunLoader(machine, options);
  const runs = new RunCoordinator({
    engine: new XStateEngine(),
    store: new JsonlRunStore(directory),
    loader,
  });
  return { runs, runId: await runs.launch('fixture'), loads: () => loader.loads };
};
