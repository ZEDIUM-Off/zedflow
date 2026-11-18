/**
 * Deliberately fixed JavaScript for pnpm's CJS readPackage API. Keep its bytes independent of
 * TypeScript transpilation: pnpm records this file's checksum in frozen dependency locks.
 * This first-party validator never imports code or accesses files; third-party hooks stay excluded.
 */
export const DEPENDENCY_POLICY = String.raw`'use strict';
module.exports = {
  hooks: {
    readPackage: (manifest) => {
      for (const field of ['dependencies', 'optionalDependencies', 'peerDependencies']) {
        const dependencies = manifest[field];
        if (dependencies === undefined) {
          continue;
        }
        if (dependencies === null || typeof dependencies !== 'object' || Array.isArray(dependencies)) {
          throw new Error('ZF_REGISTRY_ONLY: invalid ' + field);
        }
        for (const [name, range] of Object.entries(dependencies)) {
          if (!/^(?:@[a-z0-9][a-z0-9._-]*\/)?[a-z0-9][a-z0-9._-]*$/.test(name) ||
              typeof range !== 'string' || !/^[A-Za-z0-9_*^~<>=|.+ -]+$/.test(range)) {
            throw new Error('ZF_REGISTRY_ONLY: rejected ' + name + ' in ' + field);
          }
        }
      }
      return manifest;
    },
  },
};
`;
