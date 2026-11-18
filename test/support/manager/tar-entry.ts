export interface TarEntry {
  readonly path: string;
  readonly content: string;
}

const writePath = (header: Buffer, path: string): void => {
  if (Buffer.byteLength(path) <= 100) {
    header.write(path, 0, 100);
    return;
  }
  const separator = path.lastIndexOf('/', 155);
  const prefix = path.slice(0, separator);
  const name = path.slice(separator + 1);
  if (separator < 0 || Buffer.byteLength(prefix) > 155 || Buffer.byteLength(name) > 100) {
    throw new Error('Fixture tar path does not fit a ustar header');
  }
  header.write(name, 0, 100);
  header.write(prefix, 345, 155);
};

const tarHeader = (path: string, size: number): Buffer => {
  const header = Buffer.alloc(512);
  writePath(header, path);
  header.write('0000644\0', 100, 8);
  header.write('0000000\0', 108, 8);
  header.write('0000000\0', 116, 8);
  header.write(`${size.toString(8).padStart(11, '0')}\0`, 124, 12);
  header.write('00000000000\0', 136, 12);
  header.fill(0x20, 148, 156);
  header[156] = 0x30;
  header.write('ustar\0', 257, 6);
  header.write('00', 263, 2);
  return header;
};

// Build deliberately unsafe ustar headers without an extractor rewriting fixture paths.
export const tarEntry = (entry: TarEntry): Buffer => {
  const content = Buffer.from(entry.content);
  const header = tarHeader(entry.path, content.length);
  const checksum = header.reduce((sum, byte) => sum + byte, 0);
  header.write(`${checksum.toString(8).padStart(6, '0')}\0 `, 148, 8);
  const padded = Buffer.alloc(Math.ceil(content.length / 512) * 512);
  content.copy(padded);
  return Buffer.concat([header, padded]);
};
