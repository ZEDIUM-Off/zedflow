import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { getCurrentTools } from '@earendil-works/pi-ai';
import { expect, test } from 'vite-plus/test';
import { Command } from '../../packages/cli/src/index.ts';
import { fixture, isolatedAuth } from '../support/cli/fixture.ts';
import { scriptedProvider } from '../support/cli/scripted-provider.ts';
import { guardNetwork } from '../support/cli/network-guard.ts';

guardNetwork();

test.each(['direct', 'only'] as const)(
  'CLI composition runs native model → %s tools → model',
  async (mode) => {
    const f = await fixture();
    const fake = scriptedProvider(mode);
    let output = '';
    const command = new Command({
      provider: fake.provider,
      authContext: isolatedAuth,
      interaction: { prompt: async () => 'synthetic-key', notify: () => undefined },
      write: (text) => {
        output += text;
      },
      error: (text) => {
        output += text;
      },
    });
    expect(await command.run([...f.flags, 'auth', 'login', '--api-key'])).toBe(0);
    const result = await command.run([
      ...f.flags,
      'run',
      'agent',
      '--prompt',
      'Write the fixture',
      '--tools',
      'write',
      '--codemode',
      mode === 'only' ? 'only' : 'on',
    ]);
    expect(result, output).toBe(0);
    expect(await readFile(join(f.cwd, 'generated.txt'), 'utf8')).toBe('native fixture');
    expect(output).toContain('Native turn complete');
    expect(fake.contexts).toHaveLength(2);
    const context = fake.contexts[0];
    if (!context) {
      throw new Error('No native request');
    }
    expect(getCurrentTools(context.messages).map((tool) => tool.name)).toEqual(
      mode === 'only' ? ['codemode'] : ['write', 'codemode'],
    );
  },
);
