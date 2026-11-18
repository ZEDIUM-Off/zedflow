import assert from 'node:assert/strict';
import { createGrepTool, createFindTool } from '../../../packages/tools/src/index.ts';
import { NativeSearchFixture } from './native-search-fixture.ts';
import { invoke, text } from './invocation.ts';
import { SearchErrorReasonProbe, searchErrorReasons } from './search-error-reason.ts';
import type { SearchErrorReason } from './search-error-reason.ts';

const [mode, name, root, rg, fd] = process.argv.slice(2);
assert(mode && searchErrorReasons.includes(name as SearchErrorReason) && root && rg && fd);
const box = new NativeSearchFixture(root, { rg, fd });
const probe = new SearchErrorReasonProbe(name as SearchErrorReason);
const controller = new AbortController();
const updates: unknown[] = [];
const operations = {
  isDirectory: async (): Promise<boolean> => {
    if (mode === 'reject') {
      throw probe.value;
    }
    controller.abort(probe.value);
    return true;
  },
};
if (mode.startsWith('pre-')) {
  controller.abort(probe.value);
}
const tool =
  mode === 'pre-find' ? createFindTool(box.cwd) : createGrepTool(box.cwd, { operations });
const result = await invoke(tool, { pattern: 'needle' }, controller.signal, (update) =>
  updates.push(update),
);
assert(result.isError);
assert.equal(text(result), mode === 'reject' ? `Path not found: ${box.cwd}` : probe.expected);
assert.deepEqual(probe.observations, probe.expectedObservations());
assert.deepEqual(updates, []);
assert.deepEqual(await box.launches(), []);
if (mode !== 'reject') {
  assert.equal(controller.signal.reason, probe.value);
}
const followup = await invoke(createGrepTool(box.cwd), { pattern: 'needle' });
assert(!followup.isError);
assert(text(followup).includes('notes.txt'));
assert.deepEqual(await box.launches(), ['path:rg']);
assert.deepEqual(updates, []);
console.log(
  JSON.stringify({
    mode,
    name,
    hostUsable: true,
    observations: probe.observations,
    error: text(result),
  }),
);
