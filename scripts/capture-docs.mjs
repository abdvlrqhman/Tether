// Capture the real UI with no active host, invitations, credentials, or client data.
import { chromium, expect } from '@playwright/test';
import { createServer } from 'vite';
import { mkdir } from 'node:fs/promises';
const server = await createServer({ server: { host: '127.0.0.1', port: 1421, strictPort: true } });
await server.listen();
let browser;
try {
  browser = await chromium.launch({
    executablePath:
      process.env.PLAYWRIGHT_BROWSER_PATH ||
      (process.platform === 'win32'
        ? 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe'
        : undefined),
  });
  const page = await browser.newPage({
    viewport: { width: 1440, height: 1040 },
    deviceScaleFactor: 1,
  });
  await mkdir('docs/images', { recursive: true });
  await page.goto('http://127.0.0.1:1421');
  await expect(page.getByRole('heading', { name: 'Share your workspace' })).toBeVisible();
  await page.getByLabel('Starting folder').fill('C:\\Projects\\Client');
  await page.getByRole('heading', { name: 'Share your workspace' }).click();
  await page.screenshot({
    path: 'docs/images/host-workspace.png',
    fullPage: true,
    animations: 'disabled',
  });
  await page.getByRole('button', { name: 'Connect to a device', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Connect to a workspace' })).toBeVisible();
  await page.screenshot({
    path: 'docs/images/operator-workspace.png',
    fullPage: true,
    animations: 'disabled',
  });
  await page.getByRole('button', { name: 'Agent access', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Copy agent prompt', exact: true })).toBeVisible();
  await page.screenshot({
    path: 'docs/images/agent-access.png',
    fullPage: true,
    animations: 'disabled',
  });
  await page.getByRole('button', { name: 'Software updates', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Software updates', exact: true })).toBeVisible();
  await page.screenshot({
    path: 'docs/images/settings.png',
    fullPage: true,
    animations: 'disabled',
  });
  console.log('Captured host, operator, MCP, and settings interface previews in docs/images.');
} finally {
  await browser?.close();
  await server.close();
}
