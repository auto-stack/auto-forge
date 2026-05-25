import { createApp } from 'vue'
import App from './App.vue'
import './styles/theme.css'
import 'markstream-vue/index.css'
import './components/editors/autodown/styles/autodown-editor.css'
import { enableMermaid, isMermaidEnabled } from 'markstream-vue'

enableMermaid()
console.log('[markstream] mermaid enabled:', isMermaidEnabled())

createApp(App).mount('#app')
