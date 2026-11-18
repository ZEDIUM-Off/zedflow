import { mkdir, writeFile, chmod } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { Zedflow } from '../../packages/cli/src/index.ts';
import { terminalText } from '../../packages/cli/src/terminal-text.ts';
import { ToolCatalog } from '@zf/capabilities';
import { CommandTool } from '../../packages/capabilities/src/command-tool.ts';
import { fixture, isolatedAuth } from '../support/cli/fixture.ts';
import { guardNetwork } from '../support/cli/network-guard.ts';

guardNetwork();

test('qualified command tools have distinct native names and bounded output', async () => {
  const f = await fixture();
  const tool = new CommandTool({
    id: 'review/check',
    cwd: f.cwd,
    declaration: {
      name: 'check',
      inputSchema: { type: 'object' },
      execution: {
        command: process.execPath,
        args: ['-e', `process.stdout.write('x'.repeat(200000))`],
      },
    },
  });
  expect(tool.declaration.name).toBe('review__check');
  const result = await tool.execute({
    call: { type: 'toolCall', id: 'call', name: 'review__check', arguments: {} },
    signal: new AbortController().signal,
  });
  expect(result.isError).toBe(false);
  expect(JSON.stringify(result).length).toBeLessThan(51000);
  expect(result.details).toMatchObject({ truncated: true, totalBytes: 200000 });
});

test('candidate configuration failure preserves the old native run and generation', async () => {
  const f = await fixture();
  const app = new Zedflow({
    cwd: f.cwd,
    globalDirectory: f.globalDirectory,
    authContext: isolatedAuth,
  });
  const id = await app.launch('counter');
  app.runs.send(id, { type: 'INCREMENT' });
  await app.configuration.set('project', 'tools', ['missing-tool']);
  await expect(
    app.runs.reload(id, { wait: true, signal: new AbortController().signal }),
  ).rejects.toThrow('Unknown tool');
  app.runs.send(id, { type: 'INCREMENT' });
  expect(app.runs.view(id)).toMatchObject({ context: { count: 2 } });
  expect(app.runs.records(id).some((record) => record.kind === 'replacement')).toBe(false);
  app.runs.stop(id);
});

test('terminal output strips C0/C1 controls while retaining newline and Unicode', () => {
  expect(terminalText('safe\u001b]52;secret\u0007\u009b31m\n漢字')).toBe('safe]52;secret31m\n漢字');
});

test('a command implementation change affects the prepared generation fingerprint', async () => {
  const f = await fixture();
  const directory = join(f.cwd, '.zedflow', 'tools', 'review', 'check');
  await mkdir(directory, { recursive: true });
  const declaration = (text: string) =>
    JSON.stringify({
      name: 'check',
      inputSchema: { type: 'object' },
      execution: {
        command: process.execPath,
        args: ['-e', `console.log(${JSON.stringify(text)})`],
      },
    });
  await writeFile(join(directory, 'tool.json'), declaration('first'));
  const app = new Zedflow({
    cwd: f.cwd,
    globalDirectory: f.globalDirectory,
    authContext: isolatedAuth,
  });
  const first = await app.resources.load('machine:counter');
  await writeFile(join(directory, 'tool.json'), declaration('second'));
  const second = await app.resources.load('machine:counter');
  expect(second.revision).not.toBe(first.revision);
});

test('a command terminated by a signal is a native error result', async () => {
  const f = await fixture();
  const tool = new CommandTool({
    id: 'crash',
    cwd: f.cwd,
    declaration: {
      name: 'crash',
      inputSchema: { type: 'object' },
      execution: { command: process.execPath, args: ['-e', "process.kill(process.pid,'SIGKILL')"] },
    },
  });
  const result = await tool.execute({
    call: { type: 'toolCall', id: 'kill', name: 'crash', arguments: {} },
    signal: new AbortController().signal,
  });
  expect(result.isError).toBe(true);
  expect(result.details).toMatchObject({ exitCode: null, signal: 'SIGKILL', complete: true });
});

test('native actor errors produce a nonzero real CLI exit code', async () => {
  const f = await fixture();
  const dir = join(f.cwd, '.zedflow', 'machines', 'broken');
  await mkdir(dir, { recursive: true });
  await writeFile(
    join(dir, 'machine.json'),
    JSON.stringify({
      name: 'broken',
      schemaVersion: 1,
      description: 'error fixture',
      entry: 'machine.ts',
    }),
  );
  await writeFile(
    join(dir, 'machine.ts'),
    `import {createMachine} from 'xstate';export default createMachine({id:'broken',version:'1',initial:'ready',states:{ready:{on:{FAIL:()=>{throw new Error('fixture actor failure')}}}}});`,
  );
  const result = await f.run(['run', 'broken', '--event', '{"type":"FAIL"}']);
  expect(result.code).toBe(1);
  expect(result.stderr).toContain('Native actor failed');
});

test('relative global tool paths are resolved against the declaration, not a project impostor', async () => {
  const f = await fixture();
  const dir = join(f.globalDirectory, 'tools', 'check');
  await mkdir(dir, { recursive: true });
  await writeFile(
    join(dir, 'tool.json'),
    JSON.stringify({
      name: 'check',
      inputSchema: { type: 'object' },
      execution: { command: './check.sh', args: [] },
    }),
  );
  await writeFile(join(dir, 'check.sh'), '#!/bin/sh\nprintf GLOBAL');
  await chmod(join(dir, 'check.sh'), 0o700);
  await writeFile(join(f.cwd, 'check.sh'), '#!/bin/sh\nprintf IMPOSTOR');
  await chmod(join(f.cwd, 'check.sh'), 0o700);
  const declaration = (await new ToolCatalog({ roots: [f.globalDirectory] }).list()).get('check');
  if (!declaration) {
    throw new Error('Missing tool');
  }
  const tool = new CommandTool({ id: 'check', declaration, cwd: f.cwd });
  const result = await tool.execute({
    call: { type: 'toolCall', id: 'path', name: 'check', arguments: {} },
    signal: new AbortController().signal,
  });
  expect(result.content).toEqual([{ type: 'text', text: 'GLOBAL' }]);
});
