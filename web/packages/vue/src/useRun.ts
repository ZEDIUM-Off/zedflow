import { onScopeDispose, shallowRef, toValue, watch, type MaybeRefOrGetter } from 'vue'
import type { FollowRunOptions, LiveStatus, RunFollower, RunProjectionState, RunScope, ZedflowClient } from '@zedflow/sdk'

export interface RunTarget extends RunScope { initial?: RunProjectionState }
export type UseRunOptions = Omit<FollowRunOptions, 'runId' | 'workspaceId' | 'initial' | 'transport' | 'receive' | 'status'> & {
  receive?(state: RunProjectionState): void
  status?(status: LiveStatus): void
}
/** Vue owns the subscription lifecycle; projection and catch-up remain in the SDK. */
export function useRun(client: Pick<ZedflowClient, 'followRun'>, target: MaybeRefOrGetter<RunTarget | null | undefined>, options: UseRunOptions = {}) {
  const state = shallowRef<RunProjectionState>(), status = shallowRef<LiveStatus>({ transport: 'offline' })
  let follower: RunFollower | undefined, generation = 0
  const stop = watch(() => {
    const next = toValue(target)
    // Read the identities, so mutations of a reactive target also end the old scope.
    return next ? { runId: next.runId, workspaceId: next.workspaceId, initial: next.initial } : next
  }, (next, _previous, cleanup) => {
    const request = ++generation
    follower = undefined; state.value = undefined; status.value = { transport: 'offline' }
    if (!next) return
    state.value = next.initial
    const handle = client.followRun({ ...options, ...next,
      receive(value) { if (request === generation) { state.value = value; options.receive?.(value) } },
      status(value) { if (request === generation) { status.value = value; options.status?.(value) } },
    })
    follower = handle
    cleanup(() => { ++generation; handle.close(); if (follower === handle) follower = undefined })
  }, { immediate: true, flush: 'sync' })
  onScopeDispose(stop)
  return { state, status, refresh: () => follower?.refresh(), prependTimeline: (...args: Parameters<RunFollower['prependTimeline']>) => follower?.prependTimeline(...args) }
}
