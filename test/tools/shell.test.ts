import { scriptResults } from '../../packages/tools/src/script-results.ts';
import { chmod, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import { expect, test, vi } from 'vite-plus/test';
import { createBashTool } from '../../packages/tools/src/bash.ts';
import { fixture, invoke, text } from '../support/tools/invocation.ts';

type BashOptions = NonNullable<Parameters<typeof createBashTool>[1]>;

test('bash executes real processes, streams native updates and retains structured nonzero exits', async () => {
  const cwd = await fixture();
  const updates: ToolResultMessage[] = [];
  const tool = createBashTool(cwd);
  const result = await invoke(
    tool,
    { command: 'printf "hello"; printf "error" >&2; exit 7' },
    new AbortController().signal,
    (update) => updates.push(update),
  );
  expect(result.isError).toBe(true);
  expect(text(result)).toContain('Command exited with code 7');
  expect(scriptResults.get(result)).toMatchObject({ exit_code: 7, output: 'helloerror' });
  expect(result.details ?? {}).not.toHaveProperty('structuredContent');
  expect(updates.length).toBeGreaterThan(0);
  expect(updates[0]).toMatchObject({ role: 'toolResult', toolCallId: 'test-call' });
});

test('bash truncates with UTF-8-safe tails and saves the full output privately', async () => {
  const cwd = await fixture();
  const result = await invoke(createBashTool(cwd), {
    command: "printf 'é%.0s' {1..30000}",
  });
  expect(result.isError).toBe(false);
  const details = result.details as { fullOutputPath: string; truncation: { truncated: boolean } };
  expect(details.truncation.truncated).toBe(true);
  expect(await readFile(details.fullOutputPath, 'utf8')).toBe('é'.repeat(30_000));
  expect((await stat(details.fullOutputPath)).mode & 0o777).toBe(0o600);
  expect(text(result)).not.toContain('\uFFFD');
  await rm(details.fullOutputPath);
});

test('bash cancellation and deadline terminate process trees and return explicit errors', async () => {
  const cwd = await fixture();
  const tool = createBashTool(cwd);
  const controller = new AbortController();
  const running = invoke(
    tool,
    { command: 'printf started; sleep 1; printf BAD > cancelled' },
    controller.signal,
  );
  await delay(100);
  controller.abort();
  expect(text(await running)).toContain('aborted');
  const timedOut = await invoke(tool, {
    command: 'sleep 1; printf BAD > timed-out',
    timeout: 0.05,
  });
  expect(timedOut.isError).toBe(true);
  expect(text(timedOut)).toContain('timed out');
  await delay(1100);
  await expect(stat(join(cwd, 'cancelled'))).rejects.toThrow();
  await expect(stat(join(cwd, 'timed-out'))).rejects.toThrow();
  expect((await invoke(tool, { command: 'echo never', timeout: -1 })).isError).toBe(true);
});

test('direct shell history stays bounded while script-only output is ephemeral', async () => {
  const cwd = await fixture();
  const result = await invoke(createBashTool(cwd), {
    command: `${JSON.stringify(process.execPath)} -e 'process.stdout.write("q".repeat(200000))'`,
  });
  expect(result.isError).toBe(false);
  expect(scriptResults.get(result)).toMatchObject({ output: 'q'.repeat(200000) });
  expect(text(result).length).toBeLessThan(52000);
  // Upstream truncation metadata also retains its bounded 50 KiB excerpt, not all output.
  expect(JSON.stringify(result).length).toBeLessThan(110000);
  expect(JSON.stringify(result)).not.toContain('structuredContent');
});

test('bash accepts explicitly undefined native shell settings and typed optional variables', async () => {
  expect(createBashTool.length).toBe(2);
  const cwd = await fixture();
  const literal = createBashTool(cwd, { commandPrefix: undefined, shellPath: undefined });
  expect(text(await invoke(literal, { command: 'printf literal' }))).toBe('literal');
  const execute = async (commandPrefix: string | undefined, shellPath: string | undefined) =>
    invoke(createBashTool(cwd, { commandPrefix, shellPath }), {
      command: 'printf "%s" "${P7_PREFIX:-default}"',
    });
  expect(text(await execute(undefined, undefined))).toBe('default');
  expect(text(await execute('export P7_PREFIX=overridden', '/bin/bash'))).toBe('overridden');
});

test('bash preserves an explicit native shell path, command prefix and spawn hook', async () => {
  const cwd = await fixture();
  const shellPath = join(cwd, 'shell');
  await writeFile(shellPath, '#!/bin/sh\nprintf "%s" custom-shell\nexec /bin/bash "$@"\n');
  await chmod(shellPath, 0o755);
  const tool = createBashTool(cwd, {
    shellPath,
    commandPrefix: 'export P7_PREFIX=prefix',
    exposeSessionEnvironment: false,
    spawnHook: (context) => ({
      ...context,
      command: `${context.command}; printf ":%s" "$P7_HOOK"`,
      env: { ...context.env, P7_HOOK: 'hook' },
    }),
  });
  const result = await invoke(tool, { command: 'printf ":%s" "$P7_PREFIX"' });
  expect(result.isError).toBe(false);
  expect(text(result)).toBe('custom-shell:prefix:hook');
});

test('bash forwards custom operations, spawn hooks and execution options', async () => {
  const cwd = await fixture();
  const controller = new AbortController();
  const exec = vi.fn<NonNullable<BashOptions['operations']>['exec']>(
    async (command, directory, options) => {
      expect(command).toBe('prefix\ncommand:hook');
      expect(directory).toBe(cwd);
      expect(options.signal).toBe(controller.signal);
      expect(options.timeout).toBe(2);
      expect(options.env?.['P7_HOOK']).toBe('hook');
      options.onData(Buffer.from('custom'));
      return { exitCode: 0 };
    },
  );
  const tool = createBashTool(cwd, {
    operations: { exec },
    commandPrefix: 'prefix',
    shellPath: '/does-not-exist',
    exposeSessionEnvironment: false,
    spawnHook: (context) => ({
      ...context,
      command: `${context.command}:hook`,
      env: { ...context.env, P7_HOOK: 'hook' },
    }),
  });
  const result = await invoke(tool, { command: 'command', timeout: 2 }, controller.signal);
  expect(result.isError).toBe(false);
  expect(text(result)).toBe('custom');
  expect(exec).toHaveBeenCalledTimes(1);
});

test('bash observes native option getters once in order and never reads shellPath for custom operations', async () => {
  const cwd = await fixture();
  const trace: string[] = [];
  const operations: NonNullable<BashOptions['operations']> = {
    exec: async () => ({ exitCode: 0 }),
  };
  const spawnHook: NonNullable<BashOptions['spawnHook']> = (context) => context;
  const options: BashOptions = Object.defineProperties(
    {},
    {
      operations: {
        get: () => {
          trace.push('operations');
          return operations;
        },
      },
      shellPath: {
        get: () => {
          throw new Error('shellPath must stay lazy');
        },
      },
      commandPrefix: {
        get: () => {
          trace.push('commandPrefix');
          return undefined;
        },
      },
      exposeSessionEnvironment: {
        get: () => {
          trace.push('exposeSessionEnvironment');
          return false;
        },
      },
      spawnHook: {
        get: () => {
          trace.push('spawnHook');
          return spawnHook;
        },
      },
    },
  );
  const tool = createBashTool(cwd, options);
  const observations = ['operations', 'commandPrefix', 'exposeSessionEnvironment', 'spawnHook'];
  expect(trace).toEqual(observations);
  expect((await invoke(tool, { command: 'command' })).isError).toBe(false);
  expect(trace).toEqual(observations);
});
