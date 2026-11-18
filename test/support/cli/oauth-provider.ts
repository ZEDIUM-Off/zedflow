import type { Provider } from '@earendil-works/pi-ai';
import { openaiProvider } from '@earendil-works/pi-ai/providers/openai';
import { expect } from 'vite-plus/test';

export class OAuthProvider {
  #device = '';
  readonly #provider: Provider;

  public constructor() {
    this.#provider = {
      ...openaiProvider(),
      auth: {
        oauth: {
          name: 'Fixture subscription',
          isSubscription: true,
          login: async (interaction, options) => {
            this.#device = options?.getDeviceId?.() ?? '';
            expect(await interaction.prompt({ type: 'manual_code', message: 'Code' })).toBe(
              'synthetic-code',
            );
            return {
              type: 'oauth',
              access: 'synthetic-access',
              refresh: 'synthetic-refresh',
              expires: Date.now() + 60000,
            };
          },
          refresh: async (credential) => credential,
          toAuth: async () => ({ apiKey: 'synthetic-access' }),
        },
      },
    };
  }

  public get device(): string {
    return this.#device;
  }
  public get provider(): Provider {
    return this.#provider;
  }

  public failLogin(): void {
    this.#provider.auth.oauth = {
      ...this.#provider.auth.oauth,
      name: 'Failure',
      login: async () => {
        throw new Error('synthetic-private-error');
      },
      refresh: async (credential) => credential,
      toAuth: async () => ({}),
    };
  }
}
