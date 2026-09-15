import { computed, inject, onScopeDispose, provide, ref, watch, type InjectionKey } from 'vue'
import { useClient } from '@zedflow/vue'
import { detailVersion, type ContextSnapshot, type DetailRequest, type Run, type RunScope } from '@zedflow/sdk'

function createRunDetails(current: () => Run | null) {
  const client = useClient(), revision = ref(0)
  onScopeDispose(client.details.subscribe(() => revision.value++))
  function scope(): RunScope {
    const run = current()
    if (!run?.workspaceId) throw new Error('A run workspace is required for inspection')
    return { runId: run.id, workspaceId: run.workspaceId }
  }
  async function requestRaw(run: RunScope, invocation: string) { return client.runs.requestRaw(run.runId, invocation, { workspaceId: run.workspaceId }) }
  async function download(kind: 'request' | 'tool', id: string) {
    const run = scope()
    const bytes = await (kind === 'request' ? client.runs.requestRaw(run.runId, id, { workspaceId: run.workspaceId }) : client.runs.toolOutput(run.runId, id, { workspaceId: run.workspaceId }))
    const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: 'application/octet-stream' }))
    try { const link = document.createElement('a'); link.href = url; link.download = kind === 'request' ? 'request-raw.bin' : 'tool-output.bin'; link.click() }
    finally { URL.revokeObjectURL(url) }
  }
  return {
    scope, requestRaw, download,
    entry<R extends DetailRequest>(request: R) { void revision.value; return client.details.entry(request) },
    load<R extends DetailRequest>(request: R) { return client.details.load(request) },
  }
}
const key: InjectionKey<ReturnType<typeof createRunDetails>> = Symbol('run-details')
export function provideRunDetails(current: () => Run | null) { const details = createRunDetails(current); provide(key, details); return details }
export function useRunDetails() { const details = inject(key); if (!details) throw new Error('Run details provider is required'); return details }
export { detailVersion } from '@zedflow/sdk'

export function useContextDetail(source: () => ContextSnapshot | undefined, active: () => boolean = () => true) {
  const details = useRunDetails()
  const version = computed(() => source() ? detailVersion(source()!) : undefined)
  const request = computed(() => { const value = source(); return value ? { ...details.scope(), kind: 'context' as const, id: value.invocationId, revision: version.value } : undefined })
  const entry = computed(() => request.value ? details.entry(request.value) : undefined)
  const snapshot = computed(() => {
    const index = source(), loaded = entry.value?.value
    if (!loaded) return index
    return { ...loaded, ...(index?.requestRef !== undefined ? { requestRef: index.requestRef } : {}), ...(index?.rawRef !== undefined ? { rawRef: index.rawRef } : {}), ...(index?.requestBoundary !== undefined ? { requestBoundary: index.requestBoundary } : {}), ...(index?.requestStatus !== undefined ? { requestStatus: index.requestStatus } : {}) }
  })
  function load() { if (active() && request.value) void details.load(request.value).catch(() => {}) }
  watch(() => [active(), source(), version.value], load, { immediate: true })
  return { snapshot, entry, load }
}
