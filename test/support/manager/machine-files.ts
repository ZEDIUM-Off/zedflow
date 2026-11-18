import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { Json } from '@zf/core';
import type { InstallReceipt } from '../../../packages/manager/src/index.ts';

export const manifest = (name = 'agent', overrides: Readonly<Record<string, Json>> = {}): string =>
  JSON.stringify({
    schemaVersion: 1,
    name,
    description: 'Isolated fixture machine',
    entry: 'machine.ts',
    ...overrides,
  });
export const createMachine = async (input: {
  directory: string;
  name?: string;
  entry?: string;
  overrides?: Readonly<Record<string, Json>>;
}): Promise<void> => {
  await mkdir(input.directory, { recursive: true });
  await writeFile(join(input.directory, 'machine.json'), manifest(input.name, input.overrides));
  await writeFile(
    join(input.directory, 'machine.ts'),
    input.entry ?? 'export default { fixture: true };\n',
  );
};
export const requiredReceipt = (receipts: readonly InstallReceipt[], index = 0): InstallReceipt => {
  const receipt = receipts[index];
  if (!receipt) {
    throw new Error('Missing fixture receipt');
  }
  return receipt;
};
