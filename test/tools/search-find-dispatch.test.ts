import { readdir } from 'node:fs/promises';
import { expect, test } from 'vite-plus/test';
import { fixture } from '../support/tools/invocation.ts';
import { missingExecutable } from '../support/tools/search-path.ts';
import { runFindPublicScenario } from '../support/tools/search-find-public-process.ts';

test.each([
  'custom',
  'changed-while-exists',
  'getter-throws',
  'exists-false',
  'removed-while-exists',
  'glob-aborts',
  'pre-aborted',
])(
  'public find consumes the native live discriminator and preserves caller receiver/settlement: %s',
  async (scenario) => {
    const cwd = await fixture();
    const outcomes: Record<string, string> = {
      custom: 'g2.txt',
      'changed-while-exists': 'replacement.txt',
      'getter-throws': 'getter failed',
      'exists-false': 'Path not found: <cwd>',
      'removed-while-exists': 'ops.glob is not a function',
      'glob-aborts': 'Operation aborted',
      'pre-aborted': 'This operation was aborted',
    };
    const trace: unknown[] = scenario === 'pre-aborted' ? [] : ['glob.get:1'];
    if (scenario !== 'getter-throws' && scenario !== 'pre-aborted') {
      trace.push('exists.get', 'exists.call');
    }
    if (scenario === 'custom') {
      trace.push('glob.get:2', ['glob.call', 2, '*.txt']);
    }
    if (scenario === 'changed-while-exists') {
      trace.push('replacement.call');
    }
    expect(await runFindPublicScenario(cwd, scenario)).toEqual({
      results: [
        { error: !['custom', 'changed-while-exists'].includes(scenario), text: outcomes[scenario] },
      ],
      trace,
      reads: 1,
      fetches: 0,
    });
    expect(await readdir(cwd)).toEqual([]);
  },
);

test.each(['absent-after-creation', 'null-operations'])(
  'public JavaScript find routes %s through local-only resolution with zero downloads',
  async (scenario) => {
    const cwd = await fixture();
    expect(await runFindPublicScenario(cwd, scenario)).toEqual({
      results: [{ error: true, text: missingExecutable('fd') }],
      trace: [],
      reads: 1,
      fetches: 0,
    });
    expect(await readdir(cwd)).toEqual([]);
  },
);

test('public find rechecks removed, restored and null glob on repeated tool use without downloading', async () => {
  const cwd = await fixture();
  const missing = { error: true, text: missingExecutable('fd') };
  expect(await runFindPublicScenario(cwd, 'reuse')).toEqual({
    results: [
      { error: false, text: 'g2.txt' },
      missing,
      missing,
      { error: false, text: 'g4.txt' },
      missing,
    ],
    trace: [
      'glob.get:1',
      'exists.get',
      'exists.call',
      'glob.get:2',
      ['glob.call', 2, '*.txt'],
      'glob.get:3',
      'exists.get',
      'exists.call',
      'glob.get:4',
      ['glob.call', 4, '*.txt'],
    ],
    reads: 1,
    fetches: 0,
  });
  expect(await readdir(cwd)).toEqual([]);
});

test.each(['reuse-native', 'null-operations'])(
  'public find fallback uses native Pi cache priority over PATH: %s',
  async (scenario) => {
    const { publicSearchFixture } = await import('../support/tools/search-public-process.ts');
    const box = await publicSearchFixture();
    await box.install(box.path, 'path');
    await box.install(box.bin, 'cache');
    const native = { error: false, text: 'notes.txt' };
    const observed = await runFindPublicScenario(box.cwd, scenario, {
      ...box.environment(),
      NODE_OPTIONS: '',
    });
    expect(observed).toMatchObject({
      results:
        scenario === 'reuse-native'
          ? [
              { error: false, text: 'g2.txt' },
              native,
              native,
              { error: false, text: 'g4.txt' },
              native,
            ]
          : [native],
      reads: 1,
      fetches: 0,
    });
    expect(await box.launches()).toEqual(
      Array.from({ length: scenario === 'reuse-native' ? 3 : 1 }, () => 'cache:fd'),
    );
    expect(await readdir(box.tmp)).toEqual([]);
  },
);

test.each(['getter-aborts-pending-exists', 'getter-aborts-then-throws'])(
  'public find retains Pi abort ownership before observing glob: %s',
  async (scenario) => {
    const cwd = await fixture();
    expect(await runFindPublicScenario(cwd, scenario)).toEqual({
      results: [{ error: true, text: 'Operation aborted' }],
      trace:
        scenario === 'getter-aborts-pending-exists'
          ? ['glob.get:1', 'exists.get', 'exists.call', 'settled', 'exists.completed']
          : ['glob.get:1', 'settled'],
      reads: 1,
      fetches: 0,
    });
    expect(await readdir(cwd)).toEqual([]);
  },
);
