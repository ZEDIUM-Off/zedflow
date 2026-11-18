---
description: Read before you write, split or review code, or when lint reports a limit.
---

# Code limits

Lint enforces structural limits on each TypeScript file.
`scripts/lint/limits.ts` owns each threshold.
Read that file for a number.
Run `pnpm lint` for the limits alone, or `pnpm check` for the full gate described in the [program guide](workspace-loop.md).

## Profiles

`scripts/lint/profile.ts` turns one profile of `limits.ts` into oxlint rules.
`vite.config.ts` applies the profiles by path:

| Profile | Files | Difference |
|---|---|---|
| source | Every linted file that no other profile matches, including `test/support/**` | The source thresholds apply. |
| test | `**/test/**/*.ts`, except `test/support/**` | Statements, complexity, depth, imports, classes, module functions, variables and class members are off. File and function lengths are larger. |
| machine | `**/machines/**/*.ts` | The source limits, with a larger function length and a limit on functions written inside a machine config. |

Lint reports a limit as an error. Lint skips blank lines and comment-only lines when it counts lines.

## Built-in rules

The profiles set these oxlint rules:
`max-lines`, `max-lines-per-function`, `max-statements`, `complexity`, `max-depth`, `max-params`, `import/max-dependencies` and `max-classes-per-file`.
A limit that a profile leaves unset is off in that profile.
`max-lines-per-function` counts immediately invoked functions.

## Custom rules

The `zf` plugin in `scripts/lint/plugin.ts` adds these rules:

| Rule | What it reports |
|---|---|
| `zf/max-functions-per-file` | A module-level function beyond the limit. Exported and unexported functions both count. |
| `zf/max-variables-per-scope` | A function that declares too many local variables. A destructuring pattern counts each name. A nested function and a function-valued variable do not count. |
| `zf/max-class-members` | A class body with too many lines, or too many methods. The constructor does not count. An accessor or a function-valued property counts. |
| `zf/max-machine-config-function-lines` | A function written inside the argument of `createMachine` that exceeds the limit. Name the function outside the config. |
| `zf/max-test-function-lines` | A function that exceeds the limit in the test profile. It replaces `max-lines-per-function` there. |
| `zf/index-exports-only` | A statement in an `index.ts` file that is not a re-export. An import is a statement. |
| `zf/no-rest-tuple-params` | A rest parameter that is destructured or typed as a tuple. |
| `zf/arrow-functions` | A function expression that is not an arrow function or a method, and an arrow function bound with `let` or `var`. |

`zf/max-test-function-lines` exempts the callbacks of a `describe` that the file imports from `vite-plus/test` or `vitest`.
It checks each other function, including an immediately invoked function.
A local that shadows `describe` is not exempt.

## When a limit is hit

Extract one responsibility into a class that owns its properties.
Name the class after the responsibility.
Use hard-private `#` fields, explicit member accessibility and `readonly` where possible.
The class keeps one invariant or lifecycle that the caller no longer manages.

Do not work around the limit. Each of these changes the count without changing the design:

- Splitting by line range or by name (`part1`, `helper2`, `doRun`).
- Passing an options object invented to pass `max-params`.
- Typing a rest parameter as a tuple, or destructuring it.
- Packing several statements into one expression.
- Moving work into a closure, a method table or a stateless instance wrapper.
- Repackaging former parameters as an option bag in a constructor.
- Adding a cast, an `any` or a lint disable comment.

A stateless pure helper stays a module function.
It does not replace the extraction of a responsibility.
`typescript/no-extraneous-class` rejects a class that holds only static members.

Do not change `scripts/lint/limits.ts` or the `overrides` of `vite.config.ts` to pass.
The operator owns a change of limit.
Keep the behavior and the exported names of each package `index.ts`.
