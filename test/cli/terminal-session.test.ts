import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { getCurrentTools } from '@earendil-works/pi-ai';
import { expect, test } from 'vite-plus/test';
import { fixture } from '../support/cli/fixture.ts';
import { TerminalSession } from '../support/cli/terminal-session.ts';
import { TerminalStartup } from '../support/cli/terminal-startup.ts';
import { guardNetwork } from '../support/cli/network-guard.ts';

guardNetwork();

test('CLI → actual native agent → actual Code Mode → Pi TUI streams, reloads and closes', async () => {
  const f = await fixture();
  const session = new TerminalSession(f);
  expect(await session.login()).toBe(0);
  await session.start();
  const terminal = session.terminal;
  const screen = () => session.screen();
  try {
    await expect.poll(screen).toContain('Type a prompt');
    session.submit('Write the fixture');
    await expect.poll(screen).toContain('Native turn complete');
    expect(await readFile(join(f.cwd, 'generated.txt'), 'utf8')).toBe('native fixture');
    await session.configure(['read'], 'on');
    session.submit('/reload');
    await expect.poll(screen).toContain('waiting for active work');
    expect(session.contexts).toHaveLength(2);
    session.finish();
    await expect.poll(screen).toContain('Reload applied.');
    session.submit('Next prompt');
    await expect.poll(() => session.contexts.length).toBe(3);
    const request = session.contexts[2];
    if (!request) {
      throw new Error('Missing resumed request');
    }
    expect(getCurrentTools(request.messages).map((tool) => tool.name)).toEqual([
      'read',
      'codemode',
    ]);
    expect(JSON.stringify(request)).toContain('native fixture');
    session.submit('/quit');
    expect(await session.result()).toBe(0);
    expect(terminal.stops).toBe(1);
    expect(terminal.drains).toBe(1);
    expect(terminal.writes.join('')).not.toContain('synthetic-ui-key');
  } finally {
    await session.close();
  }
});

test('a failed terminal startup sends no initial prompt or native model request', async () => {
  const f = await fixture();
  const startup = new TerminalStartup(f);
  expect(await startup.login()).toBe(0);
  startup.failStart();
  try {
    expect(await startup.run('must not execute')).toBe(1);
    expect(startup.error).toContain('terminal unavailable');
    expect(startup.contexts).toHaveLength(0);
    const records = await startup.records();
    expect(records).not.toContain('must not execute');
    expect(await startup.run(' ')).toBe(1);
    expect(startup.error).toContain('Prompt must not be empty');
  } finally {
    startup.restore();
  }
});
