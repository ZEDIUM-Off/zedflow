import type { AuthType, Models, Provider } from '@earendil-works/pi-ai';

const PREFERRED_PROVIDER = 'openai';

const METHODS: Readonly<Record<string, AuthType>> = {
  'api-key': 'api_key',
  api_key: 'api_key',
  apikey: 'api_key',
  oauth: 'oauth',
};

/** Map a `/login` argument to a native auth type; `undefined` means no argument. */
export const parseLoginMethod = (argument: string | undefined): AuthType | 'invalid' | undefined =>
  argument === undefined ? undefined : (METHODS[argument.toLowerCase()] ?? 'invalid');

/** Auth types the provider can log in with itself; ambient-only API-key auth has no login. */
export const loginMethods = (provider: Provider): readonly AuthType[] => [
  ...(provider.auth.oauth ? (['oauth'] as const) : []),
  ...(provider.auth.apiKey?.login ? (['api_key'] as const) : []),
];

export const methodLabel = (provider: Provider, method: AuthType): string =>
  method === 'oauth'
    ? (provider.auth.oauth?.loginLabel ?? provider.auth.oauth?.name ?? 'Sign in with your account')
    : (provider.auth.apiKey?.name ?? 'API key');

/** The requested provider, else the preferred or only provider with a login. */
export const chooseProvider = (
  models: Models,
  requested: string | undefined,
): { readonly provider: Provider } | { readonly problem: string } => {
  if (requested !== undefined) {
    const provider = models.getProvider(requested);
    return provider ? { provider } : { problem: 'Unknown provider' };
  }
  const candidates = models.getProviders().filter((provider) => loginMethods(provider).length > 0);
  const [only] = candidates;
  const preferred = candidates.find((provider) => provider.id === PREFERRED_PROVIDER);
  const provider = preferred ?? (candidates.length === 1 ? only : undefined);
  if (provider) {
    return { provider };
  }
  return {
    problem:
      candidates.length === 0
        ? 'No provider supports login'
        : `Several providers support login: ${candidates.map((provider) => provider.id).join(', ')}`,
  };
};
