import { appendFileSync } from 'node:fs';
import { registerHooks } from 'node:module';

// Preloaded with `node --import`; appends each resolved module URL to the file named by ZF_TRACE.
const target = process.env['ZF_TRACE'];
registerHooks({
  resolve: (specifier, context, nextResolve) => {
    const resolved = nextResolve(specifier, context);
    if (target) {
      appendFileSync(target, `${resolved.url}\n`);
    }
    return resolved;
  },
});
