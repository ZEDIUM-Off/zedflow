import { isMainThread } from 'node:worker_threads';

if (!isMainThread) {
  throw new Error('Worker startup fixture');
}
