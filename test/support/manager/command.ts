import { execFile } from 'node:child_process';
import { join } from 'node:path';
import { promisify } from 'node:util';

const execute = promisify(execFile);
export const fixtureEnv = (directory: string): NodeJS.ProcessEnv => ({
  PATH: process.env['PATH'],
  HOME: join(directory, 'home'),
  TMPDIR: join(directory, 'tmp'),
  XDG_CONFIG_HOME: join(directory, 'config'),
  GIT_CONFIG_NOSYSTEM: '1',
  GIT_CONFIG_GLOBAL: '/dev/null',
  GIT_TERMINAL_PROMPT: '0',
  LANG: 'C.UTF-8',
});
export const command = async (input: {
  directory: string;
  command: string;
  args: readonly string[];
}): Promise<string> => {
  const result = await execute(input.command, [...input.args], {
    cwd: input.directory,
    env: fixtureEnv(input.directory),
    shell: false,
  });
  return result.stdout.trim();
};
