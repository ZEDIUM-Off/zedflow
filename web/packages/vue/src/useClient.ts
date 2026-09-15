import { inject, onScopeDispose, provide, type InjectionKey } from 'vue'
import type { ZedflowClient } from '@zedflow/sdk'

export const clientKey: InjectionKey<ZedflowClient> = Symbol('zedflow-client')
/** Ownership is explicit: an injected client is borrowed, never disposed by a consumer. */
export function ownClient<T extends Pick<ZedflowClient, 'dispose'>>(client: T): T {
  onScopeDispose(() => client.dispose())
  return client
}
export function provideClient(client: ZedflowClient, owns = false): ZedflowClient {
  provide(clientKey, client)
  return owns ? ownClient(client) : client
}
export function useClient(): ZedflowClient {
  const client = inject(clientKey)
  if (!client) throw new Error('Provide a Zedflow client before using its Vue adapters')
  return client
}
