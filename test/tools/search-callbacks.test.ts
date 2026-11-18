import { readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { setImmediate } from 'node:timers/promises';
import { afterEach, expect, test, vi } from 'vite-plus/test';
import { createGrepTool } from '../../packages/tools/src/grep.ts';
import { createFindTool } from '../../packages/tools/src/find.ts';
import { nativeSearchBinary, nativeSearchFixture } from '../support/tools/native-search-fixture.ts';
import { fixture, invoke, text } from '../support/tools/invocation.ts';

const grepFixture = async () => {
  const rg = await nativeSearchBinary('rg');
  const box = await nativeSearchFixture({ rg, fd: rg });
  await box.install(box.path, 'path');
  for (const [key, value] of Object.entries(box.environment())) {
    vi.stubEnv(key, value);
  }
  return box;
};

class CallbackProbe {
  readonly #cwd: string;
  readonly #held: boolean;
  #enter: (() => void) | undefined;
  #release: (() => void) | undefined;
  readonly #waiting = new Promise<void>((resolve) => {
    this.#release = resolve;
  });
  public readonly entered = new Promise<void>((resolve) => {
    this.#enter = resolve;
  });
  public readonly trace: string[] = [];
  public readonly glob = vi.fn(() => ['never']);
  public failure = '';

  public constructor(cwd: string, held = false) {
    this.#cwd = cwd;
    this.#held = held;
  }

  public isDirectory(path: string): boolean | Promise<boolean> {
    expect(path).toBe(join(this.#cwd, 'notes.txt'));
    return this.#held ? this.#wait(false) : false;
  }

  public rejectDirectory(path: string): Promise<boolean> {
    expect(path).toBe(join(this.#cwd, 'notes.txt'));
    if (this.failure === 'rejected') {
      return Promise.reject(new Error('fixture'));
    }
    throw new Error('fixture');
  }

  public exists(path: string): Promise<boolean> {
    expect(path).toBe(this.#cwd);
    return this.#wait(true);
  }

  public release(): void {
    this.#release?.();
  }

  async #wait(value: boolean): Promise<boolean> {
    this.trace.push('entered');
    this.#enter?.();
    await this.#waiting;
    this.trace.push('released');
    if (this.failure) {
      throw new Error('late fixture');
    }
    return value;
  }
}

afterEach(() => vi.unstubAllEnvs());

test('grep preserves pending caller callback settlement on abort and never starts a late child', async () => {
  const box = await grepFixture();
  const controller = new AbortController();
  const operations = new CallbackProbe(box.cwd, true);
  const trace = operations.trace;
  const updates = vi.fn();
  const result = invoke(
    createGrepTool(box.cwd, { operations }),
    { pattern: 'needle', path: 'notes.txt' },
    controller.signal,
    updates,
  ).then((value) => {
    trace.push('settled');
    return value;
  });
  await operations.entered;
  expect(await readdir(box.tmp)).toEqual([]);
  expect(await readdir(box.bin)).toEqual([]);
  controller.abort();
  trace.push('abort');
  await setImmediate();
  expect(trace).toEqual(['entered', 'abort']);
  operations.release();
  expect(text(await result)).toBe('This operation was aborted');
  expect(trace).toEqual(['entered', 'abort', 'released', 'settled']);
  expect(await box.launches()).toEqual([]);
  expect(await readdir(box.tmp)).toEqual([]);
  expect(updates).not.toHaveBeenCalled();
});

test.each(['synchronous', 'rejected'])(
  'grep observes operations once but its live method and receiver on each call: %s',
  async (failure) => {
    const box = await grepFixture();
    let reads = 0;
    const operations = new CallbackProbe(box.cwd);
    const options = Object.defineProperty({}, 'operations', {
      get: () => {
        reads++;
        return operations;
      },
    });
    const tool = createGrepTool(box.cwd, options);
    expect(text(await invoke(tool, { pattern: 'needle', path: 'notes.txt' }))).toContain(
      'notes.txt:2: needle',
    );
    operations.failure = failure;
    operations.isDirectory = operations.rejectDirectory.bind(operations);
    expect(text(await invoke(tool, { pattern: 'needle', path: 'notes.txt' }))).toBe(
      `Path not found: ${join(box.cwd, 'notes.txt')}`,
    );
    expect(reads).toBe(1);
    expect(await readdir(box.tmp)).toEqual([]);
    expect(createGrepTool.length).toBe(2);
    expect(createFindTool.length).toBe(2);
  },
);

test('custom find keeps its native early abort settlement and never resolves executables or starts workers', async () => {
  const cwd = await fixture();
  vi.stubEnv('PATH', '');
  vi.stubEnv('PI_CODING_AGENT_DIR', join(cwd, 'no-agent'));
  vi.stubEnv('TMPDIR', join(cwd, 'nonexistent-tmp'));
  const controller = new AbortController();
  const operations = new CallbackProbe(cwd, true);
  const trace = operations.trace;
  const result = invoke(
    createFindTool(cwd, { operations }),
    { pattern: '*' },
    controller.signal,
  ).then((value) => {
    trace.push('settled');
    return value;
  });
  await operations.entered;
  controller.abort();
  trace.push('abort');
  expect(text(await result)).toBe('Operation aborted');
  expect(trace).toEqual(['entered', 'abort', 'settled']);
  operations.release();
  await setImmediate();
  expect(trace).toEqual(['entered', 'abort', 'settled', 'released']);
  expect(operations.glob).not.toHaveBeenCalled();
});

test('grep preserves a late rejected caller callback outcome after abort without leaking a worker', async () => {
  const box = await grepFixture();
  const operations = new CallbackProbe(box.cwd, true);
  const controller = new AbortController();
  const result = invoke(
    createGrepTool(box.cwd, { operations }),
    { pattern: 'needle', path: 'notes.txt' },
    controller.signal,
  );
  await operations.entered;
  controller.abort();
  operations.failure = 'rejected';
  operations.release();
  expect(text(await result)).toBe(`Path not found: ${join(box.cwd, 'notes.txt')}`);
  expect(await box.launches()).toEqual([]);
  expect(await readdir(box.tmp)).toEqual([]);
});

test('grep keeps a non-cloneable abort reason in the caller realm while its directory callback settles', async () => {
  const box = await grepFixture();
  const operations = new CallbackProbe(box.cwd, true);
  const controller = new AbortController();
  const reason = { toString: () => 'caller-owned cancellation' };
  const updates = vi.fn();
  const result = invoke(
    createGrepTool(box.cwd, { operations }),
    { pattern: 'needle', path: 'notes.txt' },
    controller.signal,
    updates,
  );
  await operations.entered;
  controller.abort(reason);
  operations.release();
  const delivered = await result;
  expect(delivered.isError).toBe(true);
  expect(text(delivered)).toBe('caller-owned cancellation');
  expect(controller.signal.reason).toBe(reason);
  expect(operations.trace).toEqual(['entered', 'released']);
  expect(await box.launches()).toEqual([]);
  expect(updates).not.toHaveBeenCalled();
});
