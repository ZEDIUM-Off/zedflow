import type { AuthType, Models, Provider } from '@earendil-works/pi-ai';
import { chooseProvider, loginMethods, methodLabel, parseLoginMethod } from './auth-selection.ts';
import { sanitizeInline } from './text.ts';
import type { TuiAuthInteraction } from './tui-auth-interaction.ts';

interface LoginSelection {
  readonly provider: Provider;
  readonly requested: AuthType | undefined;
}

const loginProblem = (provider: Provider, requested: AuthType | undefined): string | undefined => {
  const available = loginMethods(provider);
  if (requested !== undefined && !available.includes(requested)) {
    return `${sanitizeInline(provider.name, 60)} has no ${requested} login here.`;
  }
  return available.length === 0
    ? `${sanitizeInline(provider.name, 60)} is configured outside this terminal.`
    : undefined;
};

export const selectLogin = (models: Models, args: readonly string[]): LoginSelection | string => {
  const [methodArgument, providerArgument] = args;
  const requested = parseLoginMethod(methodArgument);
  if (requested === 'invalid') {
    return 'Usage: /login [api-key|oauth] [provider]';
  }
  const choice = chooseProvider(models, providerArgument);
  if ('problem' in choice) {
    return `${choice.problem}.`;
  }
  return loginProblem(choice.provider, requested) ?? { provider: choice.provider, requested };
};

export const selectLoginMethod = async (
  provider: Provider,
  interaction: TuiAuthInteraction,
): Promise<AuthType> => {
  const methods = loginMethods(provider);
  const [first] = methods;
  if (methods.length === 1 && first) {
    return first;
  }
  const chosen = await interaction.prompt({
    type: 'select',
    message: 'Choose a sign-in method',
    options: methods.map((method) => ({ id: method, label: methodLabel(provider, method) })),
  });
  const method = methods.find((candidate) => candidate === chosen);
  if (!method) {
    throw new Error('Unknown sign-in method');
  }
  return method;
};
