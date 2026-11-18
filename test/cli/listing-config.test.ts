import { stat } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { fixture } from '../support/cli/fixture.ts';
import { guardNetwork } from '../support/cli/network-guard.ts';

guardNetwork();

test('the real executable lists ordinary machines and native models without authentication', async () => {
  const f = await fixture();
  expect((await f.run(['--help'])).stdout).toContain('zf — native XState');
  const machines = await f.run(['machines']);
  expect(machines.code).toBe(0);
  expect(machines.stdout).toContain('"agent"');
  expect(machines.stdout).toContain('"counter"');
  const models = await f.run(['models']);
  expect(models.code).toBe(0);
  expect(models.stdout).toContain('gpt-6.1-sol');
  const status = await f.run(['auth', 'status']);
  expect(status.stdout).toContain('"authenticated": false');
  await expect(stat(join(f.globalDirectory, 'auth.json'))).rejects.toThrow();
});

test('the actual binary executes a generic machine with JSON events', async () => {
  const f = await fixture();
  const result = await f.run([
    'run',
    'counter',
    '--headless',
    '--event',
    '{"type":"INCREMENT"}',
    '--json',
  ]);
  expect(result.code).toBe(0);
  expect(JSON.parse(result.stdout)).toMatchObject({ value: 'ready', context: { count: 1 } });
});

test('configuration process commands preserve provenance, machine scope and secret rejection', async () => {
  const f = await fixture();
  expect((await f.run(['config', 'set', 'maxModelCalls', '2', '--machine', 'agent'])).code).toBe(0);
  const shown = await f.run(['config', 'show', 'agent']);
  expect(JSON.parse(shown.stdout)).toMatchObject({
    settings: { maxModelCalls: 2, codemode: { mode: 'on' } },
    provenance: { maxModelCalls: 'project:machine' },
  });
  const refused = await f.run(['config', 'set', 'apiKey', '"never-echo-secret"']);
  expect(refused.code).toBe(1);
  expect(refused.stdout + refused.stderr).not.toContain('never-echo-secret');
  expect((await f.run(['config', 'unset', 'maxModelCalls', '--machine', 'agent'])).code).toBe(0);
  expect(JSON.parse((await f.run(['config', 'show'])).stdout)).toMatchObject({
    settings: { maxModelCalls: 4 },
  });
});
