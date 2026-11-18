import { createModels, ModelsError } from '@earendil-works/pi-ai';
import { openaiProvider } from '@earendil-works/pi-ai/providers/openai';
import type {
  Api,
  AuthContext,
  AuthOperationOptions,
  CredentialStore,
  Model,
  Models,
  Provider,
} from '@earendil-works/pi-ai';

export interface OpenAIRuntimeOptions {
  readonly credentials: CredentialStore;
  readonly authContext?: AuthContext;
  /** Native provider seam for offline tests; production registers openaiProvider(). */
  readonly provider?: Provider;
}

/** The error reported when Pi fails to resolve OpenAI authentication. */
const authFailure = (error: unknown, signal: AbortSignal | undefined): Error => {
  if (signal?.aborted) {
    return new Error('OpenAI authentication cancelled');
  }
  if (error instanceof ModelsError && error.code === 'oauth') {
    return new Error(
      'OpenAI subscription authentication could not be refreshed. Run zf auth login to sign in again.',
    );
  }
  return new Error(
    'OpenAI credential storage or key resolution failed. Check file permissions, settings and lock contention before using zf auth login.',
  );
};

// Pi owns discovery, authentication, refresh and streaming. This class only composes it.
export class OpenAIRuntime {
  public readonly models: Models;

  public constructor(options: OpenAIRuntimeOptions) {
    const provider = options.provider ?? openaiProvider();
    if (provider.id !== 'openai') {
      throw new Error('Only the OpenAI provider is supported');
    }
    const models = createModels({
      credentials: options.credentials,
      ...(options.authContext === undefined ? {} : { authContext: options.authContext }),
    });
    models.setProvider(provider);
    this.models = models;
  }

  public select(provider: string, id: string): Model<Api> {
    if (provider !== 'openai') {
      throw new Error('Only the OpenAI provider is supported');
    }
    const model = this.models.getModel(provider, id);
    if (!model) {
      throw new Error(`Unknown OpenAI model: ${id}. Use zf models to list native models.`);
    }
    return model;
  }

  public async requireAuth(model: Model<Api>, options: AuthOperationOptions = {}): Promise<void> {
    if (model.provider !== 'openai') {
      throw new Error('Only the OpenAI provider is supported');
    }
    try {
      if (await this.models.getAuth(model, options)) {
        return;
      }
    } catch (error) {
      throw authFailure(error, options.signal);
    }
    throw new Error(
      'OpenAI authentication is missing. Run zf auth login to use an API key or ChatGPT subscription.',
    );
  }
}
