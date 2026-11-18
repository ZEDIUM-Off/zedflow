import type { Settings } from './types.ts';

export const PROGRAM_DEFAULTS: Readonly<Settings> = Object.freeze({
  codemode: Object.freeze({ mode: 'on' }),
});
