import type { Limits } from './limits.ts';

export type LimitRule = 'off' | 'error' | ['error', Record<string, number | boolean>];

const maximum = (value: number | undefined, extra: Record<string, boolean> = {}): LimitRule =>
  value === undefined ? 'off' : ['error', { max: value, ...extra }];

/** Blank lines and comment-only lines do not count toward a line limit. */
const counted = { skipBlankLines: true, skipComments: true };

const functionLengthRules = (limits: Limits): Record<string, LimitRule> =>
  limits.ignoreDescribe === true
    ? {
        'max-lines-per-function': 'off',
        'zf/max-test-function-lines': maximum(limits.functionLines),
      }
    : {
        'max-lines-per-function': maximum(limits.functionLines, { ...counted, IIFEs: true }),
        'zf/max-test-function-lines': 'off',
      };

const classMemberRule = (limits: Limits): LimitRule =>
  limits.classLines === undefined || limits.classMethods === undefined
    ? 'off'
    : ['error', { lines: limits.classLines, methods: limits.classMethods }];

/** Rules that hold in every profile; they have no threshold. */
export const structuralRules: Record<string, LimitRule> = {
  'zf/index-exports-only': 'error',
  'zf/no-rest-tuple-params': 'error',
};

export const rulesFor = (limits: Limits): Record<string, LimitRule> => ({
  'max-lines': maximum(limits.fileLines, counted),
  ...functionLengthRules(limits),
  'max-params': maximum(limits.params),
  'max-statements': maximum(limits.statements),
  complexity: maximum(limits.complexity),
  'max-depth': maximum(limits.depth),
  'import/max-dependencies': maximum(limits.dependencies),
  'max-classes-per-file': maximum(limits.classesPerFile),
  'zf/max-functions-per-file': maximum(limits.moduleFunctions),
  'zf/max-variables-per-scope': maximum(limits.variables),
  'zf/max-class-members': classMemberRule(limits),
  'zf/max-machine-config-function-lines': maximum(limits.machineConfigFunctionLines),
});
