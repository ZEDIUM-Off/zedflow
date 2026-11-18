import { mkdir, mkdtemp, readFile, stat, symlink, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { setTimeout } from 'node:timers/promises';
import { createModels } from '@earendil-works/pi-ai';
import { openaiProvider } from '@earendil-works/pi-ai/providers/openai';
import { expect, test, vi } from 'vite-plus/test';
import lockfile from 'proper-lockfile';
import { JsonCredentialStore } from '@zf/storage';

const fixture = async () => {
  const base = resolve('.artifacts/cli-goal/credential-tests');
  await mkdir(base, { recursive: true });
  const directory = await mkdtemp(join(base, 'store-'));
  const path = join(directory, 'private', 'auth.json');
  return { directory, path, store: new JsonCredentialStore(path) };
};

test('native Pi credential store reads missing files without creating them, and lists metadata only', async () => {
  const f = await fixture();
  expect(await f.store.read('openai')).toBeUndefined();
  expect(await f.store.list()).toEqual([]);
  await expect(stat(f.path)).rejects.toThrow();
  await f.store.modify('openai', async () => ({ type: 'api_key', key: 'synthetic-private-value' }));
  expect(await f.store.list()).toEqual([{ providerId: 'openai', type: 'api_key' }]);
  expect((await stat(f.path)).mode & 0o777).toBe(0o600);
  await f.store.modify('openai', async () => undefined);
  expect(await f.store.read('openai')).toEqual({ type: 'api_key', key: 'synthetic-private-value' });
  await f.store.delete('openai');
  expect(await f.store.list()).toEqual([]);
});

test('two store instances serialize read-modify-write without losing independent updates', async () => {
  const f = await fixture();
  const second = new JsonCredentialStore(f.path);
  await f.store.modify('openai', async () => ({ type: 'api_key', key: '0' }));
  await Promise.all(
    Array.from({ length: 6 }, (_, index) =>
      (index % 2 ? f.store : second).modify('openai', async (current) => {
        await setTimeout(5);
        return {
          type: 'api_key',
          key: String(Number(current?.type === 'api_key' ? current.key : '0') + 1),
        };
      }),
    ),
  );
  expect(await f.store.read('openai')).toEqual({ type: 'api_key', key: '6' });
});

test('malformed credential errors do not echo contents and failed updates preserve the file', async () => {
  const f = await fixture();
  await f.store.modify('openai', async () => ({ type: 'api_key', key: 'fixture' }));
  const malformed = '{"openai":{"type":"api_key","key":"synthetic-sensitive-value" trailing';
  await writeFile(f.path, malformed);
  await expect(f.store.read('openai')).rejects.toThrow('Invalid credential JSON');
  await expect(
    f.store.modify('openai', async () => ({ type: 'api_key', key: 'replacement' })),
  ).rejects.toThrow('Invalid credential JSON');
  expect(await readFile(f.path, 'utf8')).toBe(malformed);
});

test('credential storage refuses file symlinks and pre-aborted mutations', async () => {
  const f = await fixture();
  await expect(
    f.store.modify('openai', async () => ({ type: 'api_key', key: 'fixture' }), {
      signal: AbortSignal.abort(),
    }),
  ).rejects.toThrow();
  await expect(stat(f.path)).rejects.toThrow();
  const outside = join(f.directory, 'outside.json');
  await writeFile(outside, '{}');
  await mkdir(join(f.directory, 'private'));
  await symlink(outside, f.path);
  await expect(f.store.read('openai')).rejects.toThrow('regular');
  await expect(
    f.store.modify('openai', async () => ({ type: 'api_key', key: 'fixture' })),
  ).rejects.toThrow('regular');
  expect(await readFile(outside, 'utf8')).toBe('{}');
});

test('native Pi Models refreshes OAuth once across two persistent stores and handles logout', async () => {
  const f = await fixture();
  let refreshes = 0;
  const provider = openaiProvider();
  const oauth = {
    name: 'Fixture',
    isSubscription: true,
    login: async () => ({
      type: 'oauth' as const,
      access: 'expired-fixture',
      refresh: 'refresh-fixture',
      expires: 0,
    }),
    refresh: async () => {
      refreshes += 1;
      await setTimeout(10);
      return {
        type: 'oauth' as const,
        access: 'fresh-fixture',
        refresh: 'rotated-fixture',
        expires: Date.now() + 3600000,
      };
    },
    toAuth: async (value: { access: string }) => ({ apiKey: value.access }),
  };
  const first = createModels({
    credentials: f.store,
    authContext: { env: async () => undefined, fileExists: async () => false },
  });
  const second = createModels({
    credentials: new JsonCredentialStore(f.path),
    authContext: { env: async () => undefined, fileExists: async () => false },
  });
  first.setProvider({ ...provider, auth: { ...provider.auth, oauth } });
  second.setProvider({ ...provider, auth: { ...provider.auth, oauth } });
  await first.login('openai', 'oauth', { prompt: async () => 'fixture', notify: () => {} });
  const results = await Promise.all([first.getAuth('openai'), second.getAuth('openai')]);
  expect(refreshes).toBe(1);
  expect(results.map((value) => value?.auth.apiKey)).toEqual(['fresh-fixture', 'fresh-fixture']);
  expect(await f.store.read('openai')).toMatchObject({ refresh: 'rotated-fixture' });
  await second.logout('openai');
  expect(await first.checkAuth('openai')).toBeUndefined();
});

test('a failed or cancelled credential callback leaves the original credential intact', async () => {
  const f = await fixture();
  await f.store.modify('openai', async () => ({ type: 'api_key', key: 'original-fixture' }));
  await expect(
    f.store.modify('openai', async () => {
      throw new Error('fixture failure');
    }),
  ).rejects.toThrow('fixture failure');
  const abort = new AbortController();
  await expect(
    f.store.modify(
      'openai',
      async () => {
        abort.abort();
        return { type: 'api_key', key: 'late-fixture' };
      },
      { signal: abort.signal },
    ),
  ).rejects.toThrow();
  expect(await f.store.read('openai')).toEqual({ type: 'api_key', key: 'original-fixture' });
});

test('a compromised credential lock preserves the original failure and stored credential', async () => {
  const f = await fixture();
  await f.store.modify('openai', async () => ({ type: 'api_key', key: 'original-fixture' }));
  const release = vi.fn(async () => {
    throw new Error('ERELEASED');
  });
  const locked = vi.spyOn(lockfile, 'lock').mockImplementation(async (_path, options) => {
    options?.onCompromised?.(
      Object.assign(new Error('fixture credential lock lost'), { code: 'ECOMPROMISED' }),
    );
    return release;
  });
  try {
    await expect(
      f.store.modify('openai', async () => ({ type: 'api_key', key: 'late-fixture' })),
    ).rejects.toThrow('fixture credential lock lost');
    expect(release).not.toHaveBeenCalled();
    expect(await f.store.read('openai')).toEqual({ type: 'api_key', key: 'original-fixture' });
  } finally {
    locked.mockRestore();
  }
});
