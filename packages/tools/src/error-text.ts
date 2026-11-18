/** Render once in the owning realm; neither failed conversions nor caller values escape. */
export const errorText = (error: unknown): string => {
  try {
    return String(error instanceof Error ? error.message : error);
  } catch {
    return 'Could not convert error to text';
  }
};
