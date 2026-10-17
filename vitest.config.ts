import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
export default defineConfig({
  plugins: [vue()],
  test: {
    environment: 'jsdom',
    include: ['web/tests/**/*.test.ts'],
    setupFiles: ['web/tests/setup.ts'],
  },
})
