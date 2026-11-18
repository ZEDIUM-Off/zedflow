import { isObject } from './guards.ts';
import { validateJson } from './json-validation.ts';
import type { Settings } from './types.ts';

export const validateMachineId = (id: string): void => {
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*(?:\/[a-z0-9]+(?:-[a-z0-9]+)*)*$/.test(id)) {
    throw new Error(`Invalid machine ID: ${id}`);
  }
};

const validateModel = (model: unknown): void => {
  if (model === undefined) {
    return;
  }
  if (
    !isObject(model) ||
    Object.entries(model).some(
      ([key, child]) => !['provider', 'id'].includes(key) || typeof child !== 'string' || !child,
    )
  ) {
    throw new Error('model must contain only nonempty provider/id strings');
  }
};

const validateTools = (tools: unknown): void => {
  if (
    tools !== undefined &&
    (!Array.isArray(tools) || tools.some((tool) => typeof tool !== 'string' || !tool))
  ) {
    throw new Error('tools must be an array of nonempty strings');
  }
};

const validateCodemode = (codemode: unknown): void => {
  if (
    codemode !== undefined &&
    (!isObject(codemode) ||
      Object.keys(codemode).some((key) => key !== 'mode') ||
      (codemode['mode'] !== 'on' && codemode['mode'] !== 'only'))
  ) {
    throw new Error('codemode must have mode on or only');
  }
};

const validateMaxModelCalls = (maxCalls: unknown): void => {
  if (
    maxCalls !== undefined &&
    (typeof maxCalls !== 'number' || !Number.isSafeInteger(maxCalls) || maxCalls < 1)
  ) {
    throw new Error('maxModelCalls must be a positive safe integer');
  }
};

const validateMachines = (machines: unknown, allowMachines: boolean): void => {
  if (machines === undefined) {
    return;
  }
  if (!allowMachines || !isObject(machines)) {
    throw new Error('machines must be a top-level object');
  }
  for (const [id, settings] of Object.entries(machines)) {
    validateMachineId(id);
    validateSettings(settings, false);
  }
};

export const validateSettings = (value: unknown, allowMachines = true): Settings => {
  validateJson(value);
  if (!isObject(value)) {
    throw new Error('Settings must be a JSON object');
  }
  validateModel(value['model']);
  validateTools(value['tools']);
  validateCodemode(value['codemode']);
  validateMaxModelCalls(value['maxModelCalls']);
  validateMachines(value['machines'], allowMachines);
  return value;
};
