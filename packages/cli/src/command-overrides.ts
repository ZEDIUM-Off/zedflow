import { validateSettings } from '@zf/config';
import type { Settings } from '@zf/config';
import type { parseArguments } from './command-arguments.ts';
export const commandOverrides = (values: ReturnType<typeof parseArguments>['values']): Settings => {
  const overrides: Settings = {};
  if (values.model) {
    const split = values.model.indexOf('/');
    overrides['model'] = {
      provider: split < 0 ? 'openai' : values.model.slice(0, split),
      id: split < 0 ? values.model : values.model.slice(split + 1),
    };
  }
  if (values.tools !== undefined) {
    overrides['tools'] = values.tools ? values.tools.split(',') : [];
  }
  if (values.codemode) {
    overrides['codemode'] = { mode: values.codemode };
  }
  if (values['max-model-calls']) {
    overrides['maxModelCalls'] = Number(values['max-model-calls']);
  }
  validateSettings(overrides, false);
  return overrides;
};
