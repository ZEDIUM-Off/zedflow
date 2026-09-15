import { createClient } from '@zedflow/sdk'
import { clientKey } from '@zedflow/vue'
import { CLIENT_BUILD } from './buildInfo'
import { createApp } from 'vue'
import App from './App.vue'
import '@vue-flow/core/dist/style.css'
import '@vue-flow/core/dist/theme-default.css'
import '@vue-flow/controls/dist/style.css'
import '@vue-flow/minimap/dist/style.css'
import './style.css'
import './workspace.css'
import './graph.css'
import './execution.css'
import './chrome.css'
const client = createClient({baseUrl:new URL('/api', window.location.href).href, protocol:CLIENT_BUILD.protocol})
const app = createApp(App)
app.provide(clientKey, client)
app.onUnmount(() => client.dispose())
app.mount('#app')
