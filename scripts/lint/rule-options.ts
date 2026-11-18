import type { Context, RuleOptionsSchema } from 'vite-plus/lint/plugins';

/** Schema of one rule option object whose listed keys are required non-negative integers. */
export const limitSchema = (keys: readonly string[]): RuleOptionsSchema => [
  {
    type: 'object',
    properties: Object.fromEntries(keys.map((key) => [key, { type: 'integer', minimum: 0 }])),
    required: [...keys],
    additionalProperties: false,
  },
];

/** Reads a required non-negative integer from the first option object of a rule. */
export const limitOption = (context: Context, key: string): number => {
  const [options] = context.options;
  const value =
    typeof options === 'object' && options !== null && !Array.isArray(options)
      ? options[key]
      : undefined;
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 0) {
    throw new Error(`${context.id} requires the non-negative integer option "${key}".`);
  }
  return value;
};
