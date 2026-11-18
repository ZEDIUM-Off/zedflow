import assert from 'node:assert/strict';
import { createGrepTool, createFindTool } from '../../../packages/tools/src/index.ts';
import { NativeSearchCancellation } from './native-search-cancellation.ts';
import { NativeSearchFixture } from './native-search-fixture.ts';
import { invoke, text } from './invocation.ts';

const [name, root, rg, fd] = process.argv.slice(2);
assert((name === 'grep' || name === 'find') && root && rg && fd);
const box = new NativeSearchFixture(root, { rg, fd });
// A legitimate caller-owned value that structured cloning cannot transport.
const reason = (): void => {};
const cancellation = await new NativeSearchCancellation(box, name === 'grep' ? 'rg' : 'fd').run(
  reason,
);
assert(cancellation.startedAt <= cancellation.abortedAt);
assert(cancellation.abortedAt <= cancellation.settledAt);
assert(cancellation.settledAt - cancellation.abortedAt < 5000, 'Cancellation exceeded 5 seconds');
const tool = name === 'grep' ? createGrepTool(box.cwd) : createFindTool(box.cwd);
const result = await invoke(tool, { pattern: name === 'grep' ? 'needle' : '*.txt' });
assert(!result.isError);
assert(text(result).includes('notes.txt'));
assert.deepEqual(await box.launches(), [
  `path:${name === 'grep' ? 'rg' : 'fd'}`,
  `path:${name === 'grep' ? 'rg' : 'fd'}`,
]);
console.log(
  JSON.stringify({
    name,
    hostPid: process.pid,
    cancellation,
    childGone: true,
    hostUsable: true,
    followup: text(result),
  }),
);
