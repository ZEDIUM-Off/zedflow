import type { Settings } from '@zf/config';
export const freezeSettings = (settings: Settings): Readonly<Settings> => {
  const freeze = (value: unknown): void => {
    if (value && typeof value === 'object') {
      Object.values(value).forEach(freeze);
      Object.freeze(value);
    }
  };
  const copy = structuredClone(settings);
  freeze(copy);
  return copy;
};
