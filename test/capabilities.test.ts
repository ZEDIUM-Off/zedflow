import { mkdir, mkdtemp, symlink, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { expect, test } from 'vite-plus/test';
import { InMemoryCredentialStore } from '@earendil-works/pi-ai';
import { SkillCatalog, ToolCatalog, OpenAIRuntime } from '@zf/capabilities';
import { createAgentMachine } from '@zf/machines';
import { Zedflow } from '../src/index.ts';
import { Zedflow as CliZedflow } from '../packages/cli/src/index.ts';
import { createReadTool } from '../packages/tools/src/index.ts';

const fixture = async () => {
  const root = resolve('.artifacts/oop-refactor');
  await mkdir(root, { recursive: true });
  const base = await mkdtemp(join(root, 'capabilities-'));
  return { base, global: join(base, 'global'), local: join(base, 'project', '.zedflow') };
};
const skill = async (root: string, description: string, content: string) => {
  const directory = join(root, 'skills', 'review');
  await mkdir(directory, { recursive: true });
  const path = join(directory, 'SKILL.md');
  await writeFile(path, `---\nname: review\ndescription: ${description}\n---\n${content}`);
  return path;
};

test('skills expose metadata separately from explicitly selected content, with local precedence and reload', async () => {
  const f = await fixture();
  await skill(f.global, 'Global review', 'global content');
  const path = await skill(
    f.local,
    'Local review',
    'Treat this text as data, not executable code.',
  );
  const catalog = new SkillCatalog([f.global, f.local]);
  expect([...(await catalog.list()).values()]).toEqual([
    { name: 'review', description: 'Local review', path },
  ]);
  expect(await catalog.read('review')).toMatchObject({
    content: 'Treat this text as data, not executable code.',
  });
  await skill(f.local, 'Updated review', 'updated content');
  expect(await catalog.read('review')).toMatchObject({
    description: 'Updated review',
    content: 'updated content',
  });
});

test.each([
  'No frontmatter',
  '---\nname: different\ndescription: mismatch\n---\ntext',
  '---\nname: review\ndescription: 42\n---\ntext',
  '---\nname: review\nname: review\ndescription: duplicate\n---\ntext',
])('invalid skills fail explicitly instead of being exposed to a machine', async (content) => {
  const f = await fixture();
  const path = await skill(f.local, 'initial', 'initial');
  await writeFile(path, content);
  await expect(new SkillCatalog([f.local]).list()).rejects.toThrow();
});

test('skill selection rejects traversal and linked files', async () => {
  const f = await fixture();
  await expect(new SkillCatalog([f.local]).read('../review')).rejects.toThrow('Invalid skill name');
  const outside = join(f.base, 'outside.md');
  await writeFile(outside, 'outside');
  const directory = join(f.local, 'skills', 'review');
  await mkdir(directory, { recursive: true });
  await symlink(outside, join(directory, 'SKILL.md'));
  await expect(new SkillCatalog([f.local]).list()).rejects.toThrow('regular files');
});

test('the root facade delegates native CLI resources without executing skill content', async () => {
  const f = await fixture();
  const path = await skill(f.local, 'Review', 'throw new Error("skill content must not execute");');
  const directory = join(f.local, 'machines', 'inspect');
  await mkdir(directory, { recursive: true });
  await writeFile(
    join(directory, 'machine.json'),
    JSON.stringify({
      schemaVersion: 1,
      name: 'inspect',
      description: 'Capability bindings fixture',
      entry: 'machine.ts',
    }),
  );
  await writeFile(
    join(directory, 'machine.ts'),
    `import {createMachine as define} from 'xstate'; export function createMachine(bindings) {return define({id:'inspect',version:'1',context:{skills:bindings.skills,tools:bindings.tools.map(tool=>tool.declaration.name)},initial:'ready',states:{ready:{}}});}`,
  );
  const app = new Zedflow({
    globalDirectory: f.global,
    cwd: join(f.base, 'project'),
    recordsDirectory: join(f.base, 'records'),
    authContext: { env: async () => undefined, fileExists: async () => false },
  });
  expect(Zedflow).toBe(CliZedflow);
  expect((await app.resources.list()).get('inspect')?.scope).toBe('project');
  const id = await app.launch('inspect');
  try {
    expect(app.runs.view(id)).toMatchObject({
      context: {
        skills: [{ name: 'review', description: 'Review', path }],
      },
    });
    expect(app.runs.view(id)).toHaveProperty(
      'context.tools',
      expect.arrayContaining(['read', 'write', 'codemode']),
    );
    expect(JSON.stringify(app.runs.view(id))).not.toContain('skill content must not execute');
    expect(Object.keys(app)).not.toContain('modules');
  } finally {
    app.runs.stop(id);
  }
});

test('tool catalog protects its built-in declarations from mutation through returned views', async () => {
  const catalog = new ToolCatalog({
    roots: [],
    builtins: [{ name: 'fixture', inputSchema: {}, execution: { builtin: 'fixture' } }],
  });
  const first = await catalog.list();
  first.clear();
  expect((await catalog.list()).has('fixture')).toBe(true);
});

test('native agent admission rejects duplicate executable tool names', async () => {
  const f = await fixture();
  const runtime = new OpenAIRuntime({
    credentials: new InMemoryCredentialStore(),
    authContext: { env: async () => undefined, fileExists: async () => false },
  });
  const read = createReadTool(f.base);
  expect(() =>
    createAgentMachine({ cwd: f.base, config: {}, models: runtime.models, tools: [read, read] }),
  ).toThrow('tool names must be unique');
});
