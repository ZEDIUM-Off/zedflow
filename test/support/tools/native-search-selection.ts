import assert from 'node:assert/strict';
import { mkdir, readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { createFindTool } from '../../../packages/tools/src/find.ts';
import { createGrepTool } from '../../../packages/tools/src/grep.ts';
import { nativeSearchFixture } from './native-search-fixture.ts';
import type { NativeSearchFixture } from './native-search-fixture.ts';
import { invoke, text } from './invocation.ts';
import { assertNativeSearchParity } from './search-public-process.ts';

const prepare = async (box: NativeSearchFixture, scenario: string): Promise<string[]> => {
  if (scenario === 'cache-only') {
    await box.install(box.bin, 'cache');
    return ['cache:rg', 'cache:fd'];
  }
  if (scenario.startsWith('bad-path')) {
    await box.broken(box.path, scenario === 'bad-path-nonzero' ? 'exit 42' : 'echo wrong-program');
    await box.install(box.bin, 'cache');
    return ['cache:rg', 'cache:fd'];
  }
  await box.install(box.path, 'path', scenario === 'fdfind');
  if (scenario === 'directory-cache') {
    await Promise.all(['rg', 'fd'].map((name) => mkdir(join(box.bin, name))));
  } else {
    await box.install(box.bin, 'cache');
  }
  return scenario === 'directory-cache' ? [] : ['cache:rg', 'cache:fd'];
};

class SelectionCase {
  readonly #box: NativeSearchFixture;
  readonly #scenario: string;
  #fetches = 0;

  public constructor(box: NativeSearchFixture, scenario: string) {
    this.#box = box;
    this.#scenario = scenario;
  }

  public async run(): Promise<void> {
    const expected = await prepare(this.#box, this.#scenario);
    Object.assign(process.env, this.#box.environment());
    delete process.env['PI_OFFLINE'];
    globalThis.fetch = async (): Promise<never> => {
      this.#fetches++;
      throw new Error('Unexpected fetch');
    };
    const before = { ...process.env };
    await this.#search();
    await assertNativeSearchParity(this.#box, this.#box.environment());
    assert.deepEqual(
      (await this.#box.launches()).sort(),
      [...expected, ...expected, ...expected].sort(),
    );
    assert.deepEqual(await readdir(this.#box.tmp), []);
    assert(
      JSON.stringify({ ...process.env }) === JSON.stringify(before),
      'Parent environment changed',
    );
    assert.equal(this.#fetches, 0);
    console.log(
      JSON.stringify({ scenario: this.#scenario, expected, fetches: this.#fetches, clean: true }),
    );
  }

  async #search(): Promise<void> {
    const results = await Promise.all([
      invoke(createGrepTool(this.#box.cwd), { pattern: 'needle', path: 'notes.txt', limit: 1 }),
      invoke(createFindTool(this.#box.cwd), { pattern: '*.txt' }),
    ]);
    if (this.#scenario === 'directory-cache') {
      assert(results.every((result) => result.isError));
      assert(results.every((result) => /EACCES/.test(text(result))));
      return;
    }
    assert.deepEqual(
      results.map((result) => result.isError),
      [false, false],
    );
    assert.match(text(results[0]), /^notes.txt:2: needle/);
    assert.deepEqual(results[0].details, { matchLimitReached: 1 });
    assert.equal(text(results[1]), 'notes.txt');
  }
}

const [scenario, rg, fd] = process.argv.slice(2);
assert(scenario && rg && fd);
await new SelectionCase(await nativeSearchFixture({ rg, fd }), scenario).run();
