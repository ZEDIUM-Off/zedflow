import { resolve } from 'node:path';
import { GitSourceSpec } from './git-source-spec.ts';
import { packageName, registryRange } from './registry-package.ts';
import type { Source } from './types.ts';

const parseNpmSource = (spec: string): Source => {
  const match = /^(?<name>(?:@[^/@]+\/)?[^/@]+)(?:@(?<range>.+))?$/.exec(spec);
  const groups = match?.groups;
  const name = groups?.['name'];
  const range = groups?.['range'] ?? 'latest';
  if (!name || !packageName(name) || !registryRange(range)) {
    throw new Error('Invalid npm package name/range');
  }
  return { kind: 'npm', normalized: `npm:${name}@${range}`, location: name, ref: range };
};

const parseLocalSource = (input: string): Source => {
  const local = input.startsWith('local:') ? input.slice(6) : input;
  if (!local) {
    throw new Error('Local source path is required');
  }
  const location = resolve(local);
  return { kind: 'local', location, ref: '', normalized: `local:${location}` };
};

export const parseSource = (input: string): Source => {
  if (!input || input.includes('\0') || input.includes('\n')) {
    throw new Error('Invalid source');
  }
  if (input.startsWith('npm:')) {
    return parseNpmSource(input.slice(4));
  }
  if (input.startsWith('git:')) {
    return new GitSourceSpec(input.slice(4)).source();
  }
  return parseLocalSource(input);
};
