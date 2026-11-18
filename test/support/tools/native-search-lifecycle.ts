import assert from 'node:assert/strict';
import { readdir } from 'node:fs/promises';
import { createGrepTool } from '../../../packages/tools/src/grep.ts';
import { createFindTool } from '../../../packages/tools/src/find.ts';
import { NativeSearchCancellation } from './native-search-cancellation.ts';
import { nativeSearchFixture } from './native-search-fixture.ts';
import type { NativeSearchFixture } from './native-search-fixture.ts';
import { invoke, text } from './invocation.ts';

class LifecycleCase {
  readonly #box: NativeSearchFixture;

  public constructor(box: NativeSearchFixture) {
    this.#box = box;
  }

  public async run(scenario: string): Promise<void> {
    await this.#box.install(this.#box.path, 'path');
    Object.assign(process.env, this.#box.environment());
    if (scenario === 'cancel-grep' || scenario === 'cancel-find') {
      await new NativeSearchCancellation(this.#box, scenario === 'cancel-grep' ? 'rg' : 'fd').run();
    } else {
      await this.#noLaunch(scenario);
    }
    assert.deepEqual(await readdir(this.#box.tmp), []);
    assert.deepEqual(await readdir(this.#box.bin), []);
    console.log(JSON.stringify({ scenario, clean: true }));
  }

  async #noLaunch(scenario: string): Promise<void> {
    const controller = new AbortController();
    if (scenario === 'pre-aborted') {
      controller.abort();
    }
    for (const tool of [createGrepTool(this.#box.cwd), createFindTool(this.#box.cwd)]) {
      const result = await invoke(
        tool,
        scenario === 'invalid' ? {} : { pattern: '*' },
        controller.signal,
      );
      assert(result.isError);
      if (scenario === 'startup-error') {
        assert.equal(text(result), 'Worker startup fixture');
      }
    }
    assert.deepEqual(await this.#box.launches(), []);
  }
}

const [scenario, rg, fd] = process.argv.slice(2);
assert(scenario && rg && fd);
await new LifecycleCase(await nativeSearchFixture({ rg, fd })).run(scenario);
