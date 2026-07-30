import path from 'node:path'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { configDefaults, defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { '@': path.resolve(import.meta.dirname, 'src') },
  },
  server: {
    proxy: { '/api': 'http://localhost:8080' },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts'],
    exclude: [...configDefaults.exclude, 'e2e/**', '.stryker-tmp/**'],
    // Жёсткие таймауты: зависший тест падает с ошибкой, а не висит в фоне
    testTimeout: 10_000,
    hookTimeout: 10_000,
    teardownTimeout: 5_000,
  },
})
