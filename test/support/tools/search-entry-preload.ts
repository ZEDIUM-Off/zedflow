import { appendFileSync } from 'node:fs';
import { isMainThread } from 'node:worker_threads';

if (!isMainThread) {
  const marker = process.env['SEARCH_PRELOAD_MARKER'];
  if (marker) {
    appendFileSync(marker, 'worker preload\n');
  }
  if (process.env['SEARCH_PRELOAD_FAIL'] === '1') {
    throw new Error('Search entry preload fixture');
  }
}
