import { manifest } from './machine-files.ts';
import type { PublishedPackage } from './package-archive.ts';

export const scopedMachinePackage = (version: string, entryVersion: number): PublishedPackage => ({
  name: '@fixture/machines',
  version,
  files: {
    'machine.json': manifest(),
    'machine.ts': `export { value } from 'fixture-helper'; export default { version: ${entryVersion} };`,
  },
  metadata: { dependencies: { 'fixture-helper': '1.0.0' } },
});
