import { fileURLToPath } from 'node:url';

export const BUILTIN_MACHINES_DIRECTORY = fileURLToPath(new URL('../', import.meta.url));
