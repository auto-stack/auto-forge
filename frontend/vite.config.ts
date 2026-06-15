import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'

export default defineConfig({
  base: '/forge/',
  plugins: [vue()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
  },
  optimizeDeps: {
    include: [
      'vue',
      'vue-i18n',
      'marked',
      'mermaid',
      'lucide-vue-next',
    ],
  },
  server: {
    port: 5174,
    host: '127.0.0.1',
    hmr: {
      host: '127.0.0.1',
      port: 5174,
    },
    warmup: {
      clientFiles: ['./src/main.ts', './src/App.vue', './src/views/ChatsView.vue'],
    },
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:3031',
        changeOrigin: true,
      },
      '/avatars': {
        target: 'http://127.0.0.1:3031',
        changeOrigin: true,
      },
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
  },
})
