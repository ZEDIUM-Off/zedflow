import { z } from 'zod';
import { jsonValueSchema } from '../core/json.js';
import { modelEntrySchema } from './model.js';
export const modelCatalogSchema = z.object({ models: z.array(modelEntrySchema), providers: z.array(z.object({ id: z.string(), label: z.string() }).catchall(jsonValueSchema)) }).catchall(jsonValueSchema);
export type ModelCatalog = z.output<typeof modelCatalogSchema>;
export const codexStatusSchema = z.object({ installed: z.boolean(), authenticated: z.boolean(), authMode: z.string(), message: z.string(), credentialFilePresent: z.boolean(), loginCommand: z.string(), transport: z.string() }).catchall(jsonValueSchema);
export type CodexStatus = z.output<typeof codexStatusSchema>;
