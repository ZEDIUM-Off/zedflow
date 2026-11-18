import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import type { ProcessRequest, ProcessRunner } from './types.ts';

const execute = promisify(execFile);
export class NativeProcessRunner implements ProcessRunner {
  public async run(request: ProcessRequest): Promise<string> {
    const result = await execute(request.command, [...request.args], {
      cwd: request.cwd,
      env: request.env,
      shell: false,
      timeout: 120000,
      maxBuffer: 16 * 1024 * 1024,
      encoding: 'utf8',
    });
    return result.stdout;
  }
}
