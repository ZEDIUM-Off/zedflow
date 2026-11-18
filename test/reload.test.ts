import { mkdir, mkdtemp } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { setup, types } from 'xstate';
import { expect, test } from 'vite-plus/test';
import { RunCoordinator } from '@zf/run';
import { ReloadFixture } from './support/reload/reload-fixture.ts';
import { JsonlRunStore } from '@zf/storage';
import { XStateEngine } from '@zf/xstate';
import type { RunUpdate } from '@zf/core';

import type { NativeEffect } from './support/reload/native-effect.ts';

const effectMachine = (effect: NativeEffect, increment: number, invalid: boolean) =>
  setup({
    schemas: {
      context: types<{ count: number }>(),
      events: { START: types<{ type: 'START' }>(), ADD: types<{ type: 'ADD' }>() },
    },
    actors: { effect: effect.logic },
  }).createMachine({
    id: 'effect',
    version: invalid ? 'incompatible' : '1',
    context: { count: 0 },
    initial: 'ready',
    states: {
      ready: {
        on: {
          START: { target: 'working' },
          ADD: ({ context }) => ({ context: { count: context.count + increment } }),
        },
      },
      working: {
        invoke: {
          src: 'effect',
          onDone: ({ context, event }) => ({
            target: 'ready',
            context: { count: context.count + event.output },
          }),
        },
      },
    },
  });

const fixture = () => ReloadFixture.create(effectMachine);

test('reload waits for native effects without cancellation or replay, then preserves context with new behavior', async () => {
  const f = await fixture();
  const updates: RunUpdate[] = [];
  const unsubscribe = f.coordinator.subscribe(f.id, (update) => updates.push(update));
  f.coordinator.send(f.id, { type: 'START' });
  f.change();
  const pending = f.coordinator.reload(f.id, { wait: true });
  await Promise.resolve();
  expect(f.state()).toEqual({ calls: 1, aborted: false, loads: 1 });
  expect(updates.at(-1)?.reload).toBe('waiting');
  await expect(f.coordinator.reload(f.id, { wait: true })).rejects.toThrow('Run unavailable');
  f.done(3);
  await pending;
  expect(f.state().calls).toBe(1);
  expect(f.coordinator.view(f.id)).toMatchObject({ context: { count: 3 }, value: 'ready' });
  f.coordinator.send(f.id, { type: 'ADD' });
  expect(f.coordinator.view(f.id)).toMatchObject({ context: { count: 13 } });
  await Promise.resolve();
  expect(updates.map((update) => update.reload)).toContain('preparing');
  expect(updates.at(-1)).toMatchObject({ generation: 2, reload: 'applied' });
  expect(
    f.coordinator.records(f.id).filter((record) => record.kind === 'replacement'),
  ).toHaveLength(1);
  unsubscribe();
  f.coordinator.stop(f.id);
});

test('an incompatible candidate after a deferred reload leaves the prior generation usable', async () => {
  const f = await fixture();
  f.coordinator.send(f.id, { type: 'START' });
  f.incompatible();
  const pending = f.coordinator.reload(f.id, { wait: true });
  f.done(4);
  await expect(pending).rejects.toThrow('identity/version');
  f.coordinator.send(f.id, { type: 'ADD' });
  expect(f.coordinator.view(f.id)).toMatchObject({ context: { count: 5 } });
  expect(f.coordinator.records(f.id).some((record) => record.kind === 'replacement')).toBe(false);
  f.coordinator.stop(f.id);
});

test('cancelling a pending reload does not cancel the active native effect', async () => {
  const f = await fixture();
  f.coordinator.send(f.id, { type: 'START' });
  const abort = new AbortController();
  const pending = f.coordinator.reload(f.id, { wait: true, signal: abort.signal });
  abort.abort(new Error('cancel reload request'));
  await expect(pending).rejects.toThrow('cancel reload request');
  expect(f.state()).toEqual({ calls: 1, aborted: false, loads: 1 });
  const settled = new Promise<void>((resolveReady) => {
    const unsubscribe = f.coordinator.subscribe(f.id, (update) => {
      if (
        update.view &&
        typeof update.view === 'object' &&
        !Array.isArray(update.view) &&
        update.view['value'] === 'ready'
      ) {
        unsubscribe();
        resolveReady();
      }
    });
  });
  f.done(2);
  await settled;
  expect(f.coordinator.view(f.id)).toMatchObject({ context: { count: 2 } });
  f.coordinator.stop(f.id);
});

test('explicit stopping rejects a pending reload and does not prepare a replacement', async () => {
  const f = await fixture();
  f.coordinator.send(f.id, { type: 'START' });
  const pending = f.coordinator.reload(f.id, { wait: true });
  f.coordinator.stop(f.id);
  await expect(pending).rejects.toThrow('Run unavailable');
  expect(f.state().loads).toBe(1);
  f.done(99);
});

test('observers receive detached native views and unsubscribe without controlling execution', async () => {
  const f = await fixture();
  let delivered = 0;
  const unsubscribe = f.coordinator.subscribe(f.id, (update) => {
    delivered++;
    if (update.view && typeof update.view === 'object' && !Array.isArray(update.view)) {
      update.view['context'] = null;
    }
  });
  f.coordinator.send(f.id, { type: 'ADD' });
  await Promise.resolve();
  expect(f.coordinator.view(f.id)).toMatchObject({ context: { count: 1 } });
  expect(delivered).toBeGreaterThan(1);
  unsubscribe();
  const before = delivered;
  f.coordinator.send(f.id, { type: 'ADD' });
  await Promise.resolve();
  expect(delivered).toBe(before);
  let notifications = 0;
  f.coordinator.subscribe(f.id, () => {
    if (++notifications > 1) {
      throw new Error('broken observer');
    }
  });
  f.coordinator.send(f.id, { type: 'ADD' });
  await Promise.resolve();
  f.coordinator.send(f.id, { type: 'ADD' });
  await Promise.resolve();
  expect(notifications).toBe(2);
  expect(f.coordinator.view(f.id)).toMatchObject({ context: { count: 4 } });
  f.coordinator.stop(f.id);
});

test('non-JSON views cannot crash observer notifications or leak their values', async () => {
  const root = resolve('.artifacts/cli-goal/reload-tests');
  await mkdir(root, { recursive: true });
  const path = await mkdtemp(join(root, 'non-json-'));
  const machine = setup({
    schemas: { context: types<{ count: number }>(), events: { ADD: types<{ type: 'ADD' }>() } },
  }).createMachine({
    id: 'invalid-view',
    version: '1',
    context: { count: 0 },
    initial: 'ready',
    states: { ready: { on: { ADD: () => ({ context: { count: Number.NaN } }) } } },
  });
  const coordinator = new RunCoordinator({
    engine: new XStateEngine(),
    store: new JsonlRunStore(path),
    loader: { load: async () => ({ path: 'fixture', revision: '1', module: machine }) },
  });
  const id = await coordinator.launch('fixture');
  const views: RunUpdate[] = [];
  coordinator.subscribe(id, (update) => views.push(update));
  coordinator.send(id, { type: 'ADD' });
  await Promise.resolve();
  expect(views.at(-1)?.view).toEqual({ error: 'View is not JSON-serializable' });
  expect(() => coordinator.view(id)).toThrow();
  coordinator.stop(id);
  await Promise.resolve();
  expect(views.at(-1)?.stopped).toBe(true);
});

test('a ready observer starting another effect makes deferred reload wait again', async () => {
  const f = await fixture();
  let armed = false;
  let startedAgain: (() => void) | undefined;
  const started = new Promise<void>((resolveStarted) => {
    startedAgain = resolveStarted;
  });
  f.coordinator.subscribe(f.id, (update) => {
    if (
      armed &&
      update.view &&
      typeof update.view === 'object' &&
      !Array.isArray(update.view) &&
      update.view['value'] === 'ready'
    ) {
      armed = false;
      f.coordinator.send(f.id, { type: 'START' });
      startedAgain?.();
    }
  });
  f.coordinator.send(f.id, { type: 'START' });
  armed = true;
  const pending = f.coordinator.reload(f.id, { wait: true });
  f.done(1);
  await started;
  await Promise.resolve();
  expect(f.state()).toMatchObject({ calls: 2, loads: 1 });
  f.done(2);
  await pending;
  expect(f.coordinator.view(f.id)).toMatchObject({ context: { count: 3 } });
  f.coordinator.stop(f.id);
});

test('cancellation releases a hung preparation and late candidates never activate', async () => {
  const root = resolve('.artifacts/cli-goal/reload-tests');
  await mkdir(root, { recursive: true });
  const path = await mkdtemp(join(root, 'hung-'));
  const machine = setup({}).createMachine({
    id: 'hung',
    version: '1',
    initial: 'ready',
    states: { ready: {} },
  });
  let release: (() => void) | undefined;
  let entered: (() => void) | undefined;
  let loads = 0;
  const blocked = new Promise<void>((resolveBlocked) => {
    release = resolveBlocked;
  });
  const loading = new Promise<void>((resolveLoading) => {
    entered = resolveLoading;
  });
  const coordinator = new RunCoordinator({
    engine: new XStateEngine(),
    store: new JsonlRunStore(path),
    loader: {
      load: async () => {
        if (loads++ > 0) {
          entered?.();
          await blocked;
        }
        return { path: 'fixture', revision: '1', module: machine };
      },
    },
  });
  const id = await coordinator.launch('fixture');
  const abort = new AbortController();
  const pending = coordinator.reload(id, { wait: true, signal: abort.signal });
  await loading;
  abort.abort(new Error('cancel preparation'));
  await expect(pending).rejects.toThrow('cancel preparation');
  coordinator.stop(id);
  release?.();
  await Promise.resolve();
  expect(coordinator.records(id).some((record) => record.kind === 'replacement')).toBe(false);
});
