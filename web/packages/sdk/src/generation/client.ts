import type { Transport } from '../core/transport.js';
import { validateInput } from '../core/schema.js';
import { jsonObjectSchema } from '../core/json.js';
import { compositionSchema, type Composition } from '../flows/model.js';
import { generateSchema, type GenerateInput } from './commands.js';
import { generatedSourceSchema, graphValidationSchema, buildResultSchema, type GeneratedSource, type GraphValidation, type BuildResult } from './results.js';

export function createGenerationClient(transport: Transport) {
  const sourceBody = (operation: string, input: GenerateInput) => {
    const parsed = validateInput(operation, generateSchema, input);
    // An omitted optional envelope field must not become non-JSON undefined.
    return validateInput(operation, jsonObjectSchema, Object.fromEntries(Object.entries(parsed).filter(([, value]) => value !== undefined)));
  };
  return {
    async validate(input: Composition, signal?: AbortSignal): Promise<GraphValidation> {
      const parsed = validateInput('generation.validate', compositionSchema, input);
      const body = validateInput('generation.validate', jsonObjectSchema, parsed);
      return transport.json({ operation: 'generation.validate', method: 'POST', path: 'validate', body, ...(signal ? { signal } : {}) }, graphValidationSchema);
    },
    async generate(input: GenerateInput, signal?: AbortSignal): Promise<GeneratedSource> {
      const body = sourceBody('generation.generate', input);
      return transport.json({ operation: 'generation.generate', method: 'POST', path: 'generate', body, ...(signal ? { signal } : {}) }, generatedSourceSchema);
    },
    async build(input: GenerateInput, signal?: AbortSignal): Promise<BuildResult> {
      const body = sourceBody('generation.build', input);
      return transport.json({ operation: 'generation.build', method: 'POST', path: 'build', body, ...(signal ? { signal } : {}) }, buildResultSchema);
    },
  };
}
export type GenerationClient = ReturnType<typeof createGenerationClient>;
