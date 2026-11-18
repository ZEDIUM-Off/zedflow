// Adapted from Pi 1.5.2 (MIT), copyright Mario Zechner and Earendil Works.
// Source revision b4afdddf44e059c67ccf5097e5317d913521b470; see ../PROVENANCE.json.
/**
 * Output files: files the tools write so the model can use output it was not shown in full, such as the
 * full text of truncated tool output, binary MCP resources, and images shown by codemode scripts.
 * Output files go only to the caller-scoped artifact directory, never a global temp directory.
 */
import { randomBytes } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
/** Output can carry private data, so only the user may read the files. */
const OUTPUT_FILE_MODE = 0o600;
const createOutputFilePath = (prefix: string, extension: string, directory: string): string => {
  mkdirSync(directory, { recursive: true, mode: 0o700 });
  return join(directory, `${prefix}-${randomBytes(8).toString('hex')}${extension}`);
};
/** Write `data` to a new output file and return its path. */
export const writeOutputFile = async (
  prefix: string,
  extension: string,
  data: string | Uint8Array,
  directory: string,
): Promise<string> => {
  const path = createOutputFilePath(prefix, extension, directory);
  // `wx` never follows a link someone else placed at the path.
  await writeFile(path, data, { mode: OUTPUT_FILE_MODE, flag: 'wx' });
  return path;
};
