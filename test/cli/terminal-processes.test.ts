import { readFile, stat } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { fixture } from '../support/cli/fixture.ts';
import { execute } from '../support/cli/process.ts';
import { guardNetwork } from '../support/cli/network-guard.ts';

guardNetwork();

test.each(['login', 'cancel'])(
  'the actual CLI PTY %s masks secrets and restores terminal state',
  async (mode) => {
    const f = await fixture();
    const result = await execute(
      'python3',
      [
        resolve('test/cli-pty.py'),
        process.execPath,
        resolve('packages/cli/bin/zf.ts'),
        f.cwd,
        f.globalDirectory,
        mode,
      ],
      {
        env: { PATH: process.env['PATH'], HOME: f.home, USERPROFILE: f.home, TMPDIR: f.cwd },
        timeout: 15000,
      },
    );
    const value: unknown = JSON.parse(result.stdout);
    expect(value).toMatchObject({ code: mode === 'login' ? 0 : 130, sent: true, restored: true });
    expect(result.stdout).not.toContain('synthetic-pty-key');
    if (mode === 'login') {
      expect(await readFile(join(f.globalDirectory, 'auth.json'), 'utf8')).toContain(
        'synthetic-pty-key',
      );
    } else {
      await expect(stat(join(f.globalDirectory, 'auth.json'))).rejects.toThrow();
    }
  },
);

test.each(['tui', 'tui-login'])(
  'the real CLI PTY %s runs and restores the terminal',
  async (mode) => {
    const f = await fixture();
    const result = await execute(
      'python3',
      [
        resolve('test/cli-pty.py'),
        process.execPath,
        resolve('packages/cli/bin/zf.ts'),
        f.cwd,
        f.globalDirectory,
        mode,
      ],
      {
        env: {
          PATH: process.env['PATH'],
          HOME: f.home,
          USERPROFILE: f.home,
          TMPDIR: f.cwd,
          TERM: 'xterm-256color',
        },
        timeout: 15000,
      },
    );
    const value: unknown = JSON.parse(result.stdout);
    expect(value).toMatchObject({ code: 0, phase: 3, sent: true, restored: true });
    expect(result.stdout).not.toContain('synthetic-pty-key');
    if (mode === 'tui-login') {
      expect(value).toMatchObject({ promptSafe: true });
      expect(await readFile(join(f.globalDirectory, 'auth.json'), 'utf8')).toContain(
        'synthetic-pty-key',
      );
    }
  },
);
