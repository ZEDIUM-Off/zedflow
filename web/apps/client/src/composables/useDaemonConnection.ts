import { computed, onScopeDispose, shallowRef } from 'vue'
import { useClient } from '@zedflow/vue'
import { createBrowserConnectivity, type DaemonConnectionState } from '@zedflow/sdk'

export function useDaemonConnection() {
  const state = shallowRef<DaemonConnectionState>({ health:undefined, lastSeen:undefined, connected:false, hostname:'Daemon', error:undefined })
  const connection = useClient().connectDaemon({ connectivity:createBrowserConnectivity(), receive:value=>state.value=value })
  state.value = connection.state
  onScopeDispose(connection.dispose)
  return { health:computed(()=>state.value.health), connected:computed(()=>state.value.connected), hostname:computed(()=>state.value.hostname), lastSeen:computed(()=>state.value.lastSeen), seen:connection.seen, check:connection.check }
}
