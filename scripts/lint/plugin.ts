import { definePlugin } from 'vite-plus/lint/plugins';
import { arrowFunctions } from './rules/arrow-functions.ts';
import { indexExportsOnly } from './rules/index-exports-only.ts';
import { maxClassMembers } from './rules/max-class-members.ts';
import { maxFunctionsPerFile } from './rules/max-functions-per-file.ts';
import { maxMachineConfigFunctionLines } from './rules/max-machine-config-function-lines.ts';
import { maxTestFunctionLines } from './rules/max-test-function-lines.ts';
import { maxVariablesPerScope } from './rules/max-variables-per-scope.ts';
import { noRestTupleParams } from './rules/no-rest-tuple-params.ts';

export default definePlugin({
  meta: { name: 'zf' },
  rules: {
    'arrow-functions': arrowFunctions,
    'index-exports-only': indexExportsOnly,
    'max-class-members': maxClassMembers,
    'max-functions-per-file': maxFunctionsPerFile,
    'max-machine-config-function-lines': maxMachineConfigFunctionLines,
    'max-test-function-lines': maxTestFunctionLines,
    'max-variables-per-scope': maxVariablesPerScope,
    'no-rest-tuple-params': noRestTupleParams,
  },
});
