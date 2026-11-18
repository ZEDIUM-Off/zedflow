import { expect, test } from 'vite-plus/test';
import { createMachine } from '../../packages/machines/agent/machine.ts';
import { createAgentMachine } from '../../packages/machines/src/index.ts';
import { createReadTool } from '../../packages/tools/src/index.ts';
import { AgentManifest } from '../support/agent/agent-manifest.ts';
import { fixture, context, ready } from '../support/agent/execution.ts';
import { NativeOpenAI } from '../support/agent/native-openai.ts';

test.each([
  { maxModelCalls: 0 },
  { maxModelCalls: 101 },
  { maxModelCalls: 1.5 },
  { maxModelCalls: '4' },
  { maxModelCalls: null },
  { maxToolCalls: 0 },
  { maxToolCalls: 1001 },
  { maxToolCalls: null },
  { model: null },
  { model: {} },
  { model: { provider: 'openai', id: 'not-in-catalog' } },
  { systemPrompt: 123 },
])(
  'agent validates its own machine settings without replacing the native catalog: %j',
  async (config) => {
    const cwd = await fixture();
    const native = await NativeOpenAI.create([]);
    expect(() => createMachine(native.bindings(cwd, { config }))).toThrow();
    expect(native.requestCount()).toBe(0);
  },
);

test('agent rejects duplicate executable tool names', async () => {
  const cwd = await fixture();
  const native = await NativeOpenAI.create([]);
  expect(() =>
    createMachine(native.bindings(cwd, { tools: [createReadTool(cwd), createReadTool(cwd)] })),
  ).toThrow('must be unique');
});

test('agent is an ordinary manifest resource with a named factory, native defaults and optional catalog selection', async () => {
  const cwd = await fixture();
  const manifest = await AgentManifest.create(cwd);
  const { catalog } = manifest;
  const resource = (await catalog.list()).get('agent');
  expect(resource).toMatchObject({
    id: 'agent',
    scope: 'builtin',
    manifest: {
      schemaVersion: 1,
      entry: 'machine.ts',
      defaults: {
        maxModelCalls: 4,
        maxToolCalls: 100,
        model: { provider: 'openai', id: 'gpt-6.1-sol' },
      },
      configSchema: {
        properties: { maxModelCalls: { maximum: 100 }, model: { required: ['provider', 'id'] } },
      },
    },
  });
  expect(createAgentMachine).toBe(createMachine);
  const execution = await manifest.load();
  execution.send({ type: 'PROMPT', text: 'manifest invocation' });
  await ready(execution, 1);
  expect(context(execution).messages[1]).toMatchObject({ model: 'gpt-6.1-sol' });
  const selected = manifest.alternate();
  const configured = await manifest.configured(selected);
  const { second } = manifest;
  configured.send({ type: 'PROMPT', text: 'configured' });
  await ready(configured, 1);
  expect(context(configured).messages[1]).toMatchObject({ model: selected.id });
  expect(second.request(0).messages[0]).toMatchObject({
    role: 'system',
    content: `Fixture instructions\n\nWorking directory: ${cwd}`,
  });
  const bound = manifest.bound(selected);
  bound.send({ type: 'PROMPT', text: 'bound' });
  await ready(bound, 1);
  expect(context(bound).messages[1]).toMatchObject({ model: second.model.id });
});
