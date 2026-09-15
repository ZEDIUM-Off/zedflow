import { z } from 'zod';
const model = z.string().refine(value => value.trim().length > 0, 'A model is required');
export const modelSelectionSchema = z.discriminatedUnion('provider', [
    z.strictObject({ provider: z.literal('codex'), model, reasoningEffort: z.enum(['none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max', 'ultra']).nullish(), reasoningSummary: z.enum(['auto', 'concise', 'detailed']).nullish(), textVerbosity: z.enum(['low', 'medium', 'high']).nullish() }),
    z.strictObject({ provider: z.enum(['fixture', 'gemini']), model, reasoningEffort: z.null().optional(), reasoningSummary: z.null().optional(), textVerbosity: z.null().optional() }),
]);
export type ModelSelection = z.input<typeof modelSelectionSchema>;
