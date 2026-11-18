import { expect, test } from 'vite-plus/test';
import { InspectorTransport } from './support/inspector/inspector-transport.ts';
import { createMachine } from 'xstate';
import { DevelopmentInspector } from '../dev/inspector.ts';

test('development-only SDK consumes native v6 events over an injected transport without a hosted connection', async () => {
  const transport = new InspectorTransport();
  const sent = transport.sent;
  const connection = new DevelopmentInspector(transport);
  const { inspector, engine } = connection;
  const machine = createMachine({
    id: 'dev-counter',
    version: '1',
    context: { count: 0 },
    initial: 'ready',
    states: {
      ready: { on: { ADD: ({ context }) => ({ context: { count: context.count + 1 } }) } },
    },
  });
  const execution = engine.prepare(engine.identify(machine), () => {});
  try {
    execution.start();
    expect(inspector.relayUrl).toBeNull();
    expect(inspector.inspectorUrl).toBeNull();
    expect(inspector.actors.size).toBe(1);
    const sessionId = [...inspector.actors.keys()][0];
    if (!sessionId || !transport.receive) {
      throw new Error('Inspector not connected');
    }
    transport.receive({ type: '@statelyai.system.sendEvent', sessionId, event: { type: 'ADD' } });
    expect(execution.view()).toMatchObject({ context: { count: 1 } });
    expect(sent.length).toBeGreaterThan(0);
  } finally {
    execution.stop();
    connection.dispose();
  }
  expect(transport.destroyed).toBe(false);
  expect(transport.receive).toBeUndefined();
  transport.destroy();
  expect(transport.destroyed).toBe(true);
});
