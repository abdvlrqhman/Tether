import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests',
  testMatch: 'ui.spec.ts',
  use: {
    baseURL: 'http://127.0.0.1:1420',
    viewport: { width: 1280, height: 900 },
    launchOptions: {
      executablePath:
        process.env.PLAYWRIGHT_BROWSER_PATH ||
        (process.platform === 'win32'
          ? 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe'
          : undefined),
    },
  },
  webServer: {
    command: 'npm run dev',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: !process.env.CI,
  },
  reporter: 'list',
});
