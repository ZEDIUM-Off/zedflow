import { afterEach, beforeEach, expect, vi } from 'vite-plus/test';

export const guardNetwork = (): void => {
  beforeEach(() => {
    vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('Network forbidden'));
  });
  afterEach(() => {
    expect(globalThis.fetch).not.toHaveBeenCalled();
    vi.restoreAllMocks();
  });
};
