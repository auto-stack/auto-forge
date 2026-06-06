import { createApp } from 'vue'
import App from './App.vue'
import './styles/theme.css'
import 'markstream-vue/index.css'
import './components/editors/autodown/styles/autodown-editor.css'
import { enableMermaid, isMermaidEnabled } from 'markstream-vue'
import i18n from './i18n'

enableMermaid()
console.log('[markstream] mermaid enabled:', isMermaidEnabled())

// Global fetch interceptor: auto-inject JWT for /api/ requests
const originalFetch = window.fetch
window.fetch = function (input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  const url = typeof input === 'string' ? input : input instanceof URL ? input.toString() : input.url
  if (url.startsWith('/api/')) {
    const token = localStorage.getItem('autoforge_jwt')
    if (token) {
      init = init ?? {}
      init.headers = {
        ...(init.headers ?? {}),
        Authorization: `Bearer ${token}`,
      }
    }
  }
  return originalFetch(input, init)
}

createApp(App).use(i18n).mount('#app')
