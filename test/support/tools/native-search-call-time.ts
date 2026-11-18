import assert from 'node:assert/strict';
import { readdir } from 'node:fs/promises';
import { delimiter, join, relative } from 'node:path';
import { createFindTool } from '../../../packages/tools/src/find.ts';
import { createGrepTool } from '../../../packages/tools/src/grep.ts';
import { nativeSearchFixture } from './native-search-fixture.ts';
import type { NativeSearchFixture } from './native-search-fixture.ts';
import { invoke, text } from './invocation.ts';

class CallTimeCase {
  readonly #box: NativeSearchFixture;
  readonly #tools;

  public constructor(box: NativeSearchFixture) {
    this.#box = box;
    this.#tools = [createGrepTool(box.cwd), createFindTool(box.cwd)] as const;
  }

  public async run(): Promise<void> {
    Object.assign(process.env, this.#box.environment());
    await this.#box.install(join(this.#box.root, 'later'), 'later');
    for (const stage of ['first', 'second', 'cache']) {
      await this.#stage(stage);
    }
    assert.deepEqual(await this.#box.launches(), [
      'first:rg',
      'later:fd',
      'second:rg',
      'later:fd',
      'cache:rg',
      'cache:fd',
    ]);
    assert.deepEqual(await readdir(this.#box.tmp), []);
    console.log(JSON.stringify({ callTime: true, clean: true }));
  }

  async #stage(stage: string): Promise<void> {
    const folder = join(this.#box.root, stage);
    await this.#box.install(folder, stage, true);
    if (stage === 'cache') {
      await this.#box.install(this.#box.bin, 'cache');
    }
    // Pi prefers the primary fd anywhere on PATH over an earlier fdfind alias.
    process.env['PATH'] =
      stage === 'cache'
        ? join(this.#box.root, 'missing')
        : [
            relative(process.cwd(), folder),
            relative(process.cwd(), join(this.#box.root, 'later')),
          ].join(delimiter);
    const results = [
      await invoke(this.#tools[0], { pattern: 'needle', path: 'notes.txt' }),
      await invoke(this.#tools[1], { pattern: '*.txt' }),
    ] as const;
    assert.deepEqual(
      results.map((result) => result.isError),
      [false, false],
    );
    assert.equal(text(results[0]), 'notes.txt:2: needle\nnotes.txt:4: needle');
    assert.equal(text(results[1]), 'notes.txt');
  }
}

const [rg, fd] = process.argv.slice(2);
assert(rg && fd);
await new CallTimeCase(await nativeSearchFixture({ rg, fd })).run();
