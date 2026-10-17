import { defineConfig } from '@playwright/test'
export default defineConfig({
  testDir: 'web/e2e',
  workers: 1,
  use: { baseURL: 'http://127.0.0.1:1420', viewport: { width: 1180, height: 780 } },
  webServer: {
    command: 'npm run dev',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: !process.env.CI,
  },
  reporter: 'list',
})
