import { parseArgs } from 'node:util';
import type { ParseArgsConfig } from 'node:util';
const OPTIONS = {
  help: { type: 'boolean', short: 'h' },
  json: { type: 'boolean' },
  headless: { type: 'boolean' },
  tui: { type: 'boolean' },
  stream: { type: 'boolean' },
  frozen: { type: 'boolean' },
  'api-key': { type: 'boolean' },
  cwd: { type: 'string' },
  'global-dir': { type: 'string' },
  scope: { type: 'string' },
  machine: { type: 'string' },
  prompt: { type: 'string' },
  event: { type: 'string', multiple: true },
  model: { type: 'string' },
  tools: { type: 'string' },
  codemode: { type: 'string' },
  'max-model-calls': { type: 'string' },
} satisfies ParseArgsConfig['options'];
export const parseArguments = (
  argv: readonly string[],
): ReturnType<typeof parseArgs<{ options: typeof OPTIONS; allowPositionals: true }>> => {
  try {
    return parseArgs({ args: [...argv], allowPositionals: true, options: OPTIONS });
  } catch {
    throw new Error('Invalid command arguments. Use zf --help.');
  }
};
