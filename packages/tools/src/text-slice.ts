// Adapted from Pi 1.5.2 (MIT), copyright Mario Zechner and Earendil Works.
// Source revision b4afdddf44e059c67ccf5097e5317d913521b470; see ../PROVENANCE.json.
/** Returns at most the first `maxUnits` UTF-16 code units of `text`, without splitting a surrogate pair. */
export const sliceHead = (text: string, maxUnits: number): string => {
  const end = Math.max(0, maxUnits);
  // A lone high surrogate before the cut would be invalid UTF-16; keep the whole character out instead.
  return isHighSurrogate(text.charCodeAt(end - 1)) && isLowSurrogate(text.charCodeAt(end))
    ? text.slice(0, end - 1)
    : text.slice(0, end);
};
/** Returns at most the last `maxUnits` UTF-16 code units of `text`, without splitting a surrogate pair. */
export const sliceTail = (text: string, maxUnits: number): string => {
  if (maxUnits <= 0) {
    return '';
  }
  const start = Math.max(0, text.length - maxUnits);
  return isLowSurrogate(text.charCodeAt(start)) && isHighSurrogate(text.charCodeAt(start - 1))
    ? text.slice(start + 1)
    : text.slice(start);
};
const isHighSurrogate = (code: number): boolean => {
  return code >= 0xd800 && code <= 0xdbff;
};
const isLowSurrogate = (code: number): boolean => {
  return code >= 0xdc00 && code <= 0xdfff;
};
