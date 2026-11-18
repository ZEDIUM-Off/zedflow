import { resolve } from 'node:path';
export const installationSource = (cwd: string, target: string): string =>
  target.startsWith('local:')
    ? `local:${resolve(cwd, target.slice(6))}`
    : /^(git|npm):/.test(target)
      ? target
      : `local:${resolve(cwd, target)}`;
