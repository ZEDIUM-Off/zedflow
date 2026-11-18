import type {
  LoginOptions,
  MutableModels,
  OAuthAuth,
  OAuthCredential,
} from '@earendil-works/pi-ai';
import { createModels, InMemoryCredentialStore } from '@earendil-works/pi-ai';
import { openaiProvider } from '@earendil-works/pi-ai/providers/openai';
import type { Gate } from './gate.ts';
import { gate } from './gate.ts';

export interface AuthFixture {
  readonly models: MutableModels;
  readonly credentials: InMemoryCredentialStore;
  /** Resolve the fake browser callback of the next OAuth login. */
  callback: Gate<string>;
  /** Arm a fresh callback gate for the next OAuth login. */
  arm: () => void;
  /** The OAuth login ends with this error when set. */
  failure: Error | undefined;
  /** Prompt-specific signals seen by the fake provider. */
  readonly promptSignals: AbortSignal[];
  /** Options `Models.login` passed to the fake OAuth login. */
  readonly loginOptions: (LoginOptions | undefined)[];
}

/** Own fields preserve structural copies; private methods own the native OAuth login lifecycle. */
class NativeAuthFixture implements AuthFixture {
  public readonly models: MutableModels;
  public readonly credentials: InMemoryCredentialStore;
  public callback: Gate<string>;
  public failure: Error | undefined;
  public readonly promptSignals: AbortSignal[];
  public readonly loginOptions: (LoginOptions | undefined)[];

  public constructor() {
    this.credentials = new InMemoryCredentialStore();
    this.models = createModels({
      credentials: this.credentials,
      authContext: { env: async () => undefined, fileExists: async () => false },
    });
    this.callback = gate<string>();
    this.failure = undefined;
    this.promptSignals = [];
    this.loginOptions = [];
    const native = openaiProvider();
    this.models.setProvider({ ...native, auth: { ...native.auth, oauth: this.#oauth() } });
  }

  public readonly arm = (): void => this.#arm();

  #arm(): void {
    this.callback = gate<string>();
  }

  #oauth(): OAuthAuth {
    return {
      name: 'OpenAI (ChatGPT subscription)',
      isSubscription: true,
      loginLabel: 'Sign in with ChatGPT',
      login: (interaction, options) => this.#login(interaction, options),
      refresh: async (credential) => credential,
      toAuth: async (credential) => ({ apiKey: credential.access }),
    };
  }

  async #login(
    interaction: Parameters<OAuthAuth['login']>[0],
    options?: LoginOptions,
  ): Promise<OAuthCredential> {
    this.loginOptions.push(options);
    interaction.notify({
      type: 'auth_url',
      url: 'https://auth.example.test/authorize?state=abc&code_challenge=xyz-0123456789',
      instructions: 'Complete sign-in in your browser, or paste the final redirect URL.',
    });
    const manualAbort = new AbortController();
    const signal = AbortSignal.any([manualAbort.signal, interaction.signal]);
    this.promptSignals.push(signal);
    const manual = interaction.prompt({
      type: 'manual_code',
      message: 'Complete login in your browser, or paste the final redirect URL here:',
      placeholder: 'http://localhost:1455/auth/callback',
      signal,
    });
    try {
      const code = await Promise.race([this.callback.promise, manual]);
      if (this.failure) {
        throw new Error(`${this.failure.message} for ${code}`);
      }
      interaction.notify({ type: 'progress', message: 'Exchanging authorization code...' });
      return {
        type: 'oauth',
        access: 'access-token',
        refresh: 'refresh-token',
        expires: Date.now() + 3_600_000,
      };
    } finally {
      manualAbort.abort();
      manual.catch(() => {});
    }
  }
}

/** Native API-key login plus a manual-code/browser-callback OAuth race; never uses the network. */
export const createAuthFixture = (): AuthFixture => new NativeAuthFixture();
