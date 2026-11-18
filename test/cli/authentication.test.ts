import { mkdir, readFile, writeFile, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { Command } from '../../packages/cli/src/index.ts';
import { DeviceIdentity } from '../../packages/cli/src/device-identity.ts';
import { fixture, isolatedAuth } from '../support/cli/fixture.ts';
import { OAuthProvider } from '../support/cli/oauth-provider.ts';
import { guardNetwork } from '../support/cli/network-guard.ts';

guardNetwork();

test('noninteractive login and missing model authentication fail without provider traffic', async () => {
  const f = await fixture();
  expect((await f.run(['auth', 'login', '--api-key'])).code).toBe(1);
  const result = await f.run(['run', 'agent', '--prompt', 'hello']);
  expect(result.code).toBe(1);
  expect(result.stderr).toContain('zf auth login');
});

test('native CLI API-key login/status/logout uses a private isolated store', async () => {
  const f = await fixture();
  let output = '';
  const command = new Command({
    authContext: isolatedAuth,
    interaction: { prompt: async () => 'synthetic-cli-key', notify: () => undefined },
    write: (value) => {
      output += value;
    },
    error: (value) => {
      output += value;
    },
  });
  expect(await command.run([...f.flags, 'auth', 'login', '--api-key'])).toBe(0);
  expect(await command.run([...f.flags, 'auth', 'status'])).toBe(0);
  const path = join(f.globalDirectory, 'auth.json');
  expect((await stat(path)).mode & 0o777).toBe(0o600);
  expect(await readFile(path, 'utf8')).toContain('synthetic-cli-key');
  expect(output).not.toContain('synthetic-cli-key');
  expect(await command.run([...f.flags, 'auth', 'logout'])).toBe(0);
  expect(await readFile(path, 'utf8')).not.toContain('synthetic-cli-key');
});

test('native CLI OAuth uses persistent device identity and suppresses credential/error values', async () => {
  const f = await fixture();
  const oauth = new OAuthProvider();
  let output = '';
  const command = new Command({
    provider: oauth.provider,
    authContext: isolatedAuth,
    interaction: { prompt: async () => 'synthetic-code', notify: () => undefined },
    write: (text) => {
      output += text;
    },
    error: (text) => {
      output += text;
    },
  });
  expect(await command.run([...f.flags, 'auth', 'login'])).toBe(0);
  expect(oauth.device).toBe(new DeviceIdentity(f.globalDirectory).get());
  expect(output).not.toMatch(/synthetic-(?:access|refresh|code)/);
  oauth.failLogin();
  expect(await command.run([...f.flags, 'auth', 'login'])).toBe(1);
  expect(output).not.toContain('synthetic-private-error');
});

test('auth usage/storage diagnostics and malformed machine IDs remain safe and explicit', async () => {
  const f = await fixture();
  expect((await f.run(['auth', 'wrong'])).stderr).toContain('Use auth login');
  expect((await f.run(['auth', 'login'])).stderr).toContain('interactive terminal');
  const invalid = await f.run(['config', 'set', 'maxModelCalls', '2', '--machine', 'agent.x']);
  expect(invalid.code).toBe(1);
  expect(invalid.stderr).toContain('Invalid qualified machine ID');
  await mkdir(f.globalDirectory, { recursive: true });
  await writeFile(join(f.globalDirectory, 'auth.json'), '{"secret":"never-display",broken');
  const result = await f.run(['auth', 'status']);
  expect(result.code).toBe(1);
  expect(result.stderr).toContain('credential storage');
  expect(result.stderr).not.toContain('never-display');
});
