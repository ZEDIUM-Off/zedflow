import { createModels, InMemoryCredentialStore } from '@earendil-works/pi-ai';
import type { Api, Model, Models, Provider, TranscriptContext } from '@earendil-works/pi-ai';
import { openaiProvider } from '@earendil-works/pi-ai/providers/openai';
import { convertResponsesMessages } from '@earendil-works/pi-ai/api/openai-responses-shared';
import type { MachineBindings } from '@zf/core';
import { expect } from 'vite-plus/test';

// Only the native OpenAI transport is replaced. Models, catalog, auth resolution and event streams are Pi's.
export class NativeOpenAI {
  public readonly models: Models;
  public readonly model: Model<Api>;
  readonly #requests: TranscriptContext[] = [];
  readonly #steps: readonly Provider['streamSimple'][];

  public constructor(
    credentials: InMemoryCredentialStore,
    steps: readonly Provider['streamSimple'][],
  ) {
    this.#steps = steps;
    const models = createModels({
      credentials,
      authContext: { env: async () => undefined, fileExists: async () => false },
    });
    models.setProvider({
      ...openaiProvider(),
      streamSimple: (model, context, options) => {
        const step = this.#steps[this.#requests.length];
        this.#requests.push(structuredClone(context));
        expect(options?.apiKey).toBe('offline-agent-fixture');
        if (!step) {
          throw new Error('Unexpected native model request');
        }
        return step(model, context, options);
      },
    });
    this.models = models;
    const model = models.getModel('openai', 'gpt-6.1-sol');
    if (!model) {
      throw new Error('Pinned OpenAI catalog is missing gpt-6.1-sol');
    }
    this.model = model;
  }

  public static async create(
    steps: readonly Provider['streamSimple'][],
    authenticated = true,
  ): Promise<NativeOpenAI> {
    const credentials = new InMemoryCredentialStore();
    if (authenticated) {
      await credentials.modify('openai', async () => ({
        type: 'api_key',
        key: 'offline-agent-fixture',
      }));
    }
    return new NativeOpenAI(credentials, steps);
  }
  public bindings(cwd: string, overrides: Partial<MachineBindings> = {}): MachineBindings {
    return { cwd, config: {}, models: this.models, tools: [], ...overrides };
  }
  public request(index: number): TranscriptContext {
    const request = this.#requests[index];
    if (!request) {
      throw new Error(`Missing native model request ${index}`);
    }
    return structuredClone(request);
  }
  public requestCount(): number {
    return this.#requests.length;
  }
}

export const providerInput = (
  native: NativeOpenAI,
  index: number,
): ReturnType<typeof convertResponsesMessages> =>
  convertResponsesMessages(native.model, native.request(index), new Set(['openai']));
