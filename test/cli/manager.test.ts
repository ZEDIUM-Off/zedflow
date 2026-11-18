import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { fixture } from '../support/cli/fixture.ts';
import { guardNetwork } from '../support/cli/network-guard.ts';

guardNetwork();

test('real manager commands install, list, freeze, update and remove an owned machine', async () => {
  const f = await fixture();
  const source = join(f.cwd, 'source');
  await mkdir(source);
  await writeFile(
    join(source, 'machine.json'),
    JSON.stringify({
      name: 'demo',
      schemaVersion: 1,
      description: 'CLI fixture',
      entry: 'machine.ts',
    }),
  );
  await writeFile(
    join(source, 'machine.ts'),
    `import {createMachine} from 'xstate'; export default createMachine({id:'demo',version:'1',initial:'ready',states:{ready:{}}});`,
  );
  expect((await f.run(['install', `local:${source}`])).code).toBe(0);
  expect((await f.run(['installed'])).stdout).toContain('demo');
  expect((await f.run(['restore', '--frozen'])).code).toBe(0);
  expect((await f.run(['run', 'demo', '--headless'])).code).toBe(0);
  await writeFile(join(source, 'note.txt'), 'changed source');
  expect((await f.run(['update', 'demo'])).code).toBe(0);
  expect((await f.run(['remove', 'demo'])).code).toBe(0);
  expect(JSON.parse((await f.run(['installed'])).stdout)).toEqual([]);
});
