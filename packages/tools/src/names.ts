const PORTABLE = ['read', 'write', 'edit', 'bash', 'grep', 'find', 'ls'] as const;
export const builtinToolNames = (platform: NodeJS.Platform = process.platform): readonly string[] =>
  platform === 'win32' ? [...PORTABLE, 'powershell'] : PORTABLE;
