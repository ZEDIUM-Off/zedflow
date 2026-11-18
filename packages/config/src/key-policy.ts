const unsafeKeys = new Set(['__proto__', 'prototype', 'constructor']);
const secretKeys = new Set([
  'auth',
  'authentication',
  'authorization',
  'credential',
  'credentials',
  'apikey',
  'accesstoken',
  'refreshtoken',
  'idtoken',
  'token',
  'password',
  'secret',
  'clientsecret',
  'cookie',
]);
const credentialSuffix =
  /(?:apikeys?|accesstoken|refreshtoken|idtoken|bearertoken|sessiontoken|authtoken|secret|password|privatekey|credentials?)$/;

export const isUnsafeKey = (key: string): boolean => unsafeKeys.has(key);

export const assertAllowedKey = (key: string): void => {
  if (isUnsafeKey(key) || key.includes('.')) {
    throw new Error(`Unsafe or ambiguous settings key: ${key}`);
  }
  const normalized = key.replaceAll(/[-_]/g, '').toLowerCase();
  if (secretKeys.has(normalized) || credentialSuffix.test(normalized)) {
    throw new Error(`Credentials/auth fields are not allowed in settings: ${key}`);
  }
};
