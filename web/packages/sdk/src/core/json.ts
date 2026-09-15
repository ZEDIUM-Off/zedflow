import { z } from 'zod';

/** Open JSON values keep all user channels and extension fields. */
export const jsonValueSchema = z.json();
export const jsonObjectSchema = z.record(z.string(), jsonValueSchema);
export type JsonValue = z.output<typeof jsonValueSchema>;
export type JsonObject = z.output<typeof jsonObjectSchema>;
