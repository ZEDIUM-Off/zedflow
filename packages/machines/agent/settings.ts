import type { Api, Model } from '@earendil-works/pi-ai';
import type { MachineBindings } from '@zf/core';

export interface AgentSettings {
  readonly model: Model<Api>;
  readonly maxModelCalls: number;
  readonly maxToolCalls: number;
  readonly systemPrompt: string;
}

const limit = (
  value: unknown,
  name: string,
  bounds: { readonly fallback: number; readonly maximum: number },
): number => {
  const { fallback, maximum } = bounds;
  const resolved = value === undefined ? fallback : value;
  if (
    typeof resolved !== 'number' ||
    !Number.isSafeInteger(resolved) ||
    resolved < 1 ||
    resolved > maximum
  ) {
    throw new Error(`${name} must be an integer between 1 and ${maximum}`);
  }
  return resolved;
};

const isModelSelector = (selector: unknown): selector is { provider: string; id: string } =>
  !!selector &&
  typeof selector === 'object' &&
  !Array.isArray(selector) &&
  'provider' in selector &&
  typeof selector.provider === 'string' &&
  'id' in selector &&
  typeof selector.id === 'string';

const selectModel = (bindings: MachineBindings): Model<Api> => {
  if (bindings.model) {
    return bindings.model;
  }
  const selector =
    bindings.config['model'] === undefined
      ? { provider: 'openai', id: 'gpt-6.1-sol' }
      : bindings.config['model'];
  if (!isModelSelector(selector)) {
    throw new Error('model must identify a native Pi model by provider and id');
  }
  const model = bindings.models.getModel(selector['provider'], selector['id']);
  if (!model) {
    throw new Error(`Unknown native Pi model: ${selector['provider']}/${selector['id']}`);
  }
  return model;
};

export const agentSettings = (bindings: MachineBindings): AgentSettings => {
  const systemPrompt =
    bindings.config['systemPrompt'] === undefined
      ? 'You are a coding assistant. Use the supplied tools to complete the user request.'
      : bindings.config['systemPrompt'];
  if (typeof systemPrompt !== 'string') {
    throw new Error('systemPrompt must be a string');
  }
  return {
    model: selectModel(bindings),
    maxModelCalls: limit(bindings.config['maxModelCalls'], 'maxModelCalls', {
      fallback: 4,
      maximum: 100,
    }),
    maxToolCalls: limit(bindings.config['maxToolCalls'], 'maxToolCalls', {
      fallback: 100,
      maximum: 1000,
    }),
    systemPrompt: `${systemPrompt}\n\nWorking directory: ${bindings.cwd}${bindings.skills?.length ? `\n\nAvailable skills (read their files when relevant):\n${bindings.skills.map((skill) => `- ${skill.name}: ${skill.description} (${skill.path})`).join('\n')}` : ''}`,
  };
};
