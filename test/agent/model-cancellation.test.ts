import { setTimeout as delay } from 'node:timers/promises';
import { withoutInitialSystemMessage } from '@earendil-works/pi-ai';
import type { AssistantMessageEvent } from '@earendil-works/pi-ai';
import { expect, test, vi } from 'vite-plus/test';
import { createMachine } from '../../packages/machines/agent/machine.ts';
import { DeferredModel } from '../support/agent/deferred-model.ts';
import { fixture, startMachine, context, ready } from '../support/agent/execution.ts';
import { NativeOpenAI, providerInput } from '../support/agent/native-openai.ts';
import { answer } from '../support/agent/responses.ts';

test.each(['CANCEL', 'RESET'] as const)(
  'agent %s aborts model work and suppresses late native events before reuse',
  async (event) => {
    const cwd = await fixture();
    const deferred = new DeferredModel();
    const native = await NativeOpenAI.create([deferred.stream(), answer('new response')]);
    const events: AssistantMessageEvent[] = [];
    const { execution } = startMachine(
      createMachine(native.bindings(cwd, { onModelEvent: (update) => events.push(update) })),
    );
    execution.send({ type: 'PROMPT', text: 'old prompt' });
    await vi.waitFor(() =>
      expect(events.some((update) => update.type === 'text_delta')).toBe(true),
    );
    expect(() => execution.checkpoint()).toThrow('child-free');
    execution.send({ type: event });
    expect(deferred.signal?.aborted).toBe(true);
    const stopped = execution.view();
    const count = events.length;
    deferred.finishAfterAbort();
    await delay(20);
    expect(events).toHaveLength(count);
    expect(execution.view()).toEqual(stopped);
    const history = context(execution).messages;
    execution.send({ type: 'PROMPT', text: 'new prompt' });
    await ready(execution, 1);
    expect(context(execution).messages).toHaveLength(history.length + 2);
    expect(context(execution).messages.slice(0, history.length)).toEqual(history);
    expect(context(execution).messages.at(-2)).toMatchObject({ content: 'new prompt' });
    expect(
      withoutInitialSystemMessage(native.request(1).messages).slice(0, history.length),
    ).toEqual(history);
    expect(JSON.stringify(providerInput(native, 1))).not.toContain('old response');
  },
);
