import { jsonObject } from './json-values.ts';

export const packageName = (name: string): boolean =>
  /^(?:@[a-z0-9][a-z0-9._-]*\/)?[a-z0-9][a-z0-9._-]*$/.test(name);
export const registryRange = (range: string): boolean => /^[A-Za-z0-9_*^~<>=|.+ -]+$/.test(range);
/** A dependency map whose every entry is a registry package name with a registry range. */
export const registryDependencies = (value: unknown): boolean =>
  jsonObject(value) &&
  Object.entries(value).every(
    ([name, range]) => packageName(name) && typeof range === 'string' && registryRange(range),
  );
