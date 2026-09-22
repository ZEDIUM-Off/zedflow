// Frozen public composition inputs captured at 9890994. Conversion logic is tested
// against the actual client module in web/apps/client/tests/templates.test.ts.
import { readFileSync } from 'node:fs'
import type { Composition } from '@zedflow/sdk'

function fixture(name: string): Composition {
  const value: Composition = JSON.parse(readFileSync(new URL(`${name}.json`, import.meta.url), 'utf8'))
  value.id = crypto.randomUUID()
  return value
}
export function legacyTemplate(interactive = true) { return fixture(interactive ? 'legacyInteractive' : 'legacyAutonomous') }
export function template(interactive = true) { return fixture(interactive ? 'interactive' : 'autonomous') }
export function legacyHarnessTemplate() { return fixture('legacyHarness') }
export function harnessTemplate() { return fixture('harness') }
export function parallelJoinFixture() { return fixture('parallelJoin') }
