import { mkdir, readFile, readdir, rm, stat, symlink, utimes, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';
import { Configuration, PROGRAM_DEFAULTS } from '../../packages/config/src/index.ts';
import lockfile from 'proper-lockfile';
import type { Settings } from '../../packages/config/src/index.ts';
import { ConfigurationFixture } from '../support/config/configuration-fixture.ts';

describe('Configuration public entry', () => {
  let directory: string;
  let globalDirectory: string;
  let projectDirectory: string;
  let configuration: Configuration;

  let fixture: ConfigurationFixture;

  beforeEach(async () => {
    fixture = await ConfigurationFixture.create();
    ({ directory, globalDirectory, projectDirectory, configuration } = fixture);
  });

  afterEach(async () => {
    await fixture.dispose();
  });

  it('extends the program defaults and keeps implementation state hard-private', async () => {
    const custom = new Configuration({
      globalDirectory,
      projectDirectory,
      programDefaults: { custom: true },
    });
    expect(Object.keys(custom)).toEqual([]);
    await expect(custom.resolve({ machineId: 'agent' })).resolves.toMatchObject({
      settings: { codemode: { mode: 'on' }, custom: true },
    });
  });

  it('does not echo malformed settings contents in parser errors', async () => {
    await mkdir(globalDirectory, { recursive: true });
    await writeFile(
      join(globalDirectory, 'settings.json'),
      '{"apiKey":"synthetic-sensitive-value" trailing',
    );
    await expect(configuration.read('global')).rejects.toThrow('Invalid settings JSON');
    try {
      await configuration.read('global');
    } catch (error) {
      expect(String(error)).not.toContain('synthetic-sensitive-value');
    }
  });

  it('preserves the compromised-lock diagnostic without invoking an invalid release', async () => {
    const release = vi.fn(async () => {
      throw new Error('ERELEASED');
    });
    const locked = vi.spyOn(lockfile, 'lock').mockImplementation(async (_path, options) => {
      options?.onCompromised?.(
        Object.assign(new Error('fixture lost lock'), { code: 'ECOMPROMISED' }),
      );
      return release;
    });
    try {
      await expect(configuration.set('global', 'feature', true)).rejects.toThrow(
        'Settings lock was lost:',
      );
      expect(release).not.toHaveBeenCalled();
      await expect(stat(join(globalDirectory, 'settings.json'))).rejects.toThrow();
    } finally {
      locked.mockRestore();
    }
  });

  it('recovers an abandoned stale lock using a heartbeat-aware lock protocol', async () => {
    await mkdir(globalDirectory, { recursive: true });
    const lock = join(globalDirectory, 'settings.json.lock');
    await mkdir(lock);
    const old = new Date(Date.now() - 60000);
    await utimes(lock, old, old);
    await configuration.set('global', 'feature', true);
    expect(await configuration.read('global')).toEqual({ feature: true });
  });

  it('reports live lock contention without deleting the other owner lock', async () => {
    await mkdir(globalDirectory, { recursive: true });
    const lock = join(globalDirectory, 'settings.json.lock');
    await mkdir(lock);
    await expect(configuration.set('global', 'feature', true)).rejects.toThrow(
      'Settings are locked by another writer',
    );
    expect((await stat(lock)).isDirectory()).toBe(true);
  });

  it('names the invalid scope file without echoing its contents', async () => {
    await mkdir(join(projectDirectory, '.zedflow'), { recursive: true });
    const path = join(projectDirectory, '.zedflow', 'settings.json');
    await writeFile(path, '{"apiKey":"sensitive-fixture" broken');
    await expect(configuration.resolve({ machineId: 'agent' })).rejects.toThrow(path);
    try {
      await configuration.read('project');
    } catch (error) {
      expect(String(error)).not.toContain('sensitive-fixture');
    }
  });

  it.each([
    'openaiApiKey',
    'OPENAI_API_KEY',
    'x-api-key',
    'apiKeys',
    'privateKey',
    'bearerToken',
    'sessionToken',
    'cookie',
  ])('rejects common credential field %s', async (key) => {
    await expect(configuration.set('global', 'headers', { [key]: 'fixture' })).rejects.toThrow(
      'Credentials/auth fields',
    );
  });

  it('accepts non-secret token budget settings', async () => {
    await configuration.set('global', 'maxTokens', 1000);
    expect((await configuration.read('global'))['maxTokens']).toBe(1000);
  });

  it('does not create files or directories for no-op unset', async () => {
    await configuration.unset('global', 'missing');
    await configuration.unset('project', 'missing.child');
    await expect(stat(globalDirectory)).rejects.toThrow();
    await expect(stat(join(projectDirectory, '.zedflow'))).rejects.toThrow();
    await configuration.set('global', 'present', true);
    const path = join(globalDirectory, 'settings.json');
    const before = await stat(path);
    await configuration.unset('global', 'missing');
    expect((await stat(path)).mtimeMs).toBe(before.mtimeMs);
  });

  it.each(['', '   ', '\uFEFF{}'])(
    'reads empty or BOM-prefixed settings as Pi does',
    async (content) => {
      await mkdir(globalDirectory, { recursive: true });
      await writeFile(join(globalDirectory, 'settings.json'), content);
      expect(await configuration.read('global')).toEqual({});
    },
  );

  it('reads absent files without creating them and exposes async operations', async () => {
    const reading = configuration.read('global');
    expect(reading).toBeInstanceOf(Promise);
    await expect(reading).resolves.toEqual({});
    await expect(configuration.read('project')).resolves.toEqual({});
    await expect(stat(globalDirectory)).rejects.toThrow();
    await expect(configuration.resolve({ machineId: 'domain/agent' })).resolves.toEqual({
      settings: PROGRAM_DEFAULTS,
      provenance: { 'codemode.mode': 'program' },
    });
  });

  it('preserves unrelated keys, writes exact scoped paths and unsets only the requested key', async () => {
    await configuration.set('global', 'feature', { one: 1, two: 2 });
    await configuration.set('project', 'tools', []);
    await configuration.set('global', 'feature.one', 3);
    await configuration.unset('global', 'feature.two');
    await configuration.unset('global', 'absent.child');
    const globalFile = join(globalDirectory, 'settings.json');
    expect(JSON.parse(await readFile(globalFile, 'utf8'))).toEqual({ feature: { one: 3 } });
    expect(
      JSON.parse(await readFile(join(projectDirectory, '.zedflow', 'settings.json'), 'utf8')),
    ).toEqual({ tools: [] });
    expect((await stat(globalFile)).mode & 0o777).toBe(0o600);
    expect(await readdir(globalDirectory)).toEqual(['settings.json']);
    const value = await configuration.read('global');
    value['feature'] = 'mutated';
    expect(await configuration.read('global')).toEqual({ feature: { one: 3 } });
  });

  it('serializes concurrent read-modify-write operations across instances', async () => {
    const other = new Configuration({ globalDirectory, projectDirectory });
    await Promise.all(
      Array.from({ length: 10 }, (_, index) =>
        (index % 2 === 0 ? configuration : other).set('global', `features.item${index}`, index),
      ),
    );
    expect(await configuration.read('global')).toEqual({
      features: Object.fromEntries(
        Array.from({ length: 10 }, (_, index) => [`item${index}`, index]),
      ),
    });
    expect(await readdir(globalDirectory)).toEqual(['settings.json']);
  });

  it.each(['{invalid', '[]', 'null', '{"tools":false}', '{"machines":{"../escape":{}}}'])(
    'reports malformed settings without overwriting the file: %s',
    async (content) => {
      await mkdir(globalDirectory, { recursive: true });
      const path = join(globalDirectory, 'settings.json');
      await writeFile(path, content);
      await expect(configuration.read('global')).rejects.toThrow();
      await expect(configuration.set('global', 'feature', true)).rejects.toThrow();
      await expect(configuration.unset('global', 'feature')).rejects.toThrow();
      await expect(configuration.resolve({ machineId: 'agent' })).rejects.toThrow();
      expect(await readFile(path, 'utf8')).toBe(content);
      expect(await readdir(globalDirectory)).toEqual(['settings.json']);
    },
  );

  it.each([
    'apiKey',
    'api_key',
    'credentials',
    'auth',
    'authentication',
    'authorization',
    'accessToken',
    'refresh_token',
    'password',
    'secret',
    'client-secret',
  ])('rejects credential/auth fields in every configuration layer: %s', async (key) => {
    const value: Settings = { nested: { [key]: 'fixture-not-a-real-secret' } };
    await expect(configuration.set('global', 'feature', value)).rejects.toThrow(
      /Credentials\/auth/,
    );
    await expect(configuration.set('project', `machines.agent.${key}`, 'fixture')).rejects.toThrow(
      /Credentials\/auth/,
    );
    await expect(configuration.resolve({ machineId: 'agent', defaults: value })).rejects.toThrow(
      /Credentials\/auth/,
    );
    await expect(configuration.resolve({ machineId: 'agent', overrides: value })).rejects.toThrow(
      /Credentials\/auth/,
    );
    expect(
      () => new Configuration({ globalDirectory, projectDirectory, programDefaults: value }),
    ).toThrow(/Credentials\/auth/);
    await mkdir(globalDirectory, { recursive: true });
    await writeFile(join(globalDirectory, 'settings.json'), JSON.stringify(value));
    await expect(configuration.read('global')).rejects.toThrow(/Credentials\/auth/);
  });

  it.each([
    ['tools', 'read'],
    ['tools', ['read', 1]],
    ['codemode', { mode: 'off' }],
    ['codemode', { mode: 'on', surprise: true }],
    ['model', { provider: 1 }],
    ['model', { id: '' }],
    ['model', { provider: 'openai', name: 'other' }],
    ['maxModelCalls', -1],
    ['maxModelCalls', 0],
    ['maxModelCalls', 1.5],
    ['machines', []],
    ['machines', { agent: { tools: false } }],
    ['machines', { agent: { machines: {} } }],
  ] as const)('validates reserved setting %s with value %j', async (path, value) => {
    await expect(
      configuration.set('global', path, structuredClone(value) as JsonValue),
    ).rejects.toThrow();
    expect(await configuration.read('global')).toEqual({});
  });

  it('rejects prototype pollution, ambiguous dotted keys, non-JSON values and invalid IDs', async () => {
    for (const path of [
      '__proto__.polluted',
      'constructor.prototype.polluted',
      '',
      'feature..child',
    ]) {
      await expect(configuration.set('global', path, true)).rejects.toThrow();
    }
    const polluted: unknown = JSON.parse('{"__proto__":{"polluted":true}}');
    await expect(configuration.set('global', 'feature', polluted as JsonValue)).rejects.toThrow();
    for (const value of [
      undefined,
      Number.NaN,
      Number.POSITIVE_INFINITY,
      new Date(),
      { value: undefined },
      { 'a.b': true },
    ]) {
      await expect(configuration.set('global', 'feature', value as JsonValue)).rejects.toThrow();
    }
    for (const machineId of ['../agent', '/agent', 'domain//agent', 'Agent', 'domain/./agent']) {
      await expect(configuration.resolve({ machineId })).rejects.toThrow(/Invalid machine ID/);
    }
    expect({}).not.toHaveProperty('polluted');
    await configuration.set('global', 'feature', false);
    await expect(configuration.set('global', 'feature.child', true)).rejects.toThrow(/non-object/);
    expect(await configuration.read('global')).toEqual({ feature: false });
  });

  it('rejects symlink and directory settings files without modifying their targets', async () => {
    await mkdir(globalDirectory, { recursive: true });
    const target = join(directory, 'target.json');
    await writeFile(target, '{"keep":true}');
    const path = join(globalDirectory, 'settings.json');
    await symlink(target, path);
    await expect(configuration.read('global')).rejects.toThrow(/regular file/);
    await expect(configuration.set('global', 'feature', true)).rejects.toThrow(/regular file/);
    expect(await readFile(target, 'utf8')).toBe('{"keep":true}');
    await rm(path);
    await mkdir(path);
    await expect(configuration.unset('global', 'feature')).rejects.toThrow(/regular file/);
    expect(await readdir(globalDirectory)).toEqual(['settings.json']);
  });
});

type JsonValue = Parameters<Configuration['set']>[2];
