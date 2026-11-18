import assert from 'node:assert/strict';
import { existsSync, writeFileSync } from 'node:fs';

const [marker, resource] = process.argv.slice(2);
assert(marker && resource);
// Only the owning fixture creates this resource. A late launch after release cannot stay alive.
if (existsSync(resource)) {
  writeFileSync(marker, String(process.pid));
  const lifetime = setInterval(() => {
    if (!existsSync(resource)) {
      clearInterval(lifetime);
    }
  }, 10);
}
