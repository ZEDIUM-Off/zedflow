import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';
import { Configuration } from '../../packages/config/src/index.ts';
import type { Settings } from '../../packages/config/src/index.ts';
import { ConfigurationFixture } from '../support/config/configuration-fixture.ts';

describe('Configuration public entry', () => {
  let globalDirectory: string;
  let projectDirectory: string;
  let configuration: Configuration;

  let fixture: ConfigurationFixture;

  beforeEach(async () => {
    fixture = await ConfigurationFixture.create();
    ({ globalDirectory, projectDirectory, configuration } = fixture);
  });

  afterEach(async () => {
    await fixture.dispose();
  });

  it('resolves all seven layers in order with leaf provenance and array replacement', async () => {
    await fixture.seedLayers();
    const result = await configuration.resolve({
      machineId: 'domain/agent',
      defaults: { custom: { manifest: true, a: 'manifest' }, tools: ['manifest-tool'] },
      overrides: { custom: { e: 'cli' }, tools: ['explicit'], maxModelCalls: 4 },
    });
    expect(result).toEqual({
      settings: {
        codemode: { mode: 'on' },
        custom: {
          manifest: true,
          a: 'global',
          b: 'global-machine',
          c: 'project',
          d: 'project-machine',
          e: 'cli',
        },
        tools: ['explicit'],
        maxModelCalls: 4,
      },
      provenance: {
        'codemode.mode': 'program',
        'custom.manifest': 'manifest',
        'custom.a': 'global',
        'custom.b': 'global:machine',
        'custom.c': 'project',
        'custom.d': 'project:machine',
        'custom.e': 'overrides',
        tools: 'overrides',
        maxModelCalls: 'overrides',
      },
    });
    expect(result.settings).not.toHaveProperty('machines');
  });

  it('supports composed program defaults below manifest defaults without retaining references', async () => {
    const programDefaults: Settings = { feature: { mode: 'initial' }, tools: ['program'] };
    const configured = new Configuration({ globalDirectory, projectDirectory, programDefaults });
    programDefaults['feature'] = false;
    const result = await configured.resolve({
      machineId: 'agent',
      defaults: { tools: ['manifest'] },
    });
    expect(result.settings).toEqual({
      codemode: { mode: 'on' },
      feature: { mode: 'initial' },
      tools: ['manifest'],
    });
    result.settings['feature'] = null;
    expect((await configured.resolve({ machineId: 'agent' })).settings['feature']).toEqual({
      mode: 'initial',
    });
  });

  it('merges native Pi model selectors but rejects incomplete effective selectors', async () => {
    await configuration.set('global', 'model', { provider: 'openai' });
    await expect(configuration.resolve({ machineId: 'agent' })).rejects.toThrow(
      /both provider and id/,
    );
    await configuration.set('project', 'model.id', 'fixture-model');
    expect((await configuration.resolve({ machineId: 'agent' })).settings['model']).toEqual({
      provider: 'openai',
      id: 'fixture-model',
    });
  });

  it('removes stale provenance when types change and preserves object merge semantics', async () => {
    await configuration.set('global', 'feature', { old: true });
    const replaced = await configuration.resolve({
      machineId: 'agent',
      overrides: { feature: null },
    });
    expect(replaced.provenance).not.toHaveProperty('feature.old');
    expect(replaced.provenance['feature']).toBe('overrides');
    const nested = await configuration.resolve({
      machineId: 'agent',
      defaults: { feature: false },
      overrides: { feature: { next: [] } },
    });
    expect(nested.provenance).not.toHaveProperty('feature');
    expect(nested.provenance['feature.old']).toBe('global');
    expect(nested.provenance['feature.next']).toBe('overrides');
    expect(
      (await configuration.resolve({ machineId: 'agent', defaults: { empty: {} } })).provenance[
        'empty'
      ],
    ).toBe('manifest');
  });
});
