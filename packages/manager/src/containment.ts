import { isAbsolute, relative, sep } from 'node:path';

export const contained = (root: string, path: string): boolean => {
  const child = relative(root, path);
  return child === '' || (!isAbsolute(child) && child !== '..' && !child.startsWith(`..${sep}`));
};
export const relativePath = (value: string): boolean =>
  value.length > 0 &&
  !isAbsolute(value) &&
  !value.includes('\\') &&
  value.split('/').every((segment) => segment !== '..' && segment !== '.' && segment !== '');
