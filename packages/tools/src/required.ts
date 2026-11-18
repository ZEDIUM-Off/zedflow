// Upstream loops check bounds; retain that invariant explicitly under noUncheckedIndexedAccess.
export const required = <T>(value: T | undefined): T => {
  if (value === undefined) {
    throw new Error('Internal tool invariant: missing indexed value');
  }
  return value;
};
