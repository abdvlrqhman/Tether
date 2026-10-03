import { test, expect } from '@playwright/test';
test('host workspace, operator flow, agent docs, and branding render', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Bring debugging closer.' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Start sharing' })).toBeDisabled();
  await page.getByLabel('Starting folder').fill('D:\\Projects');
  await page.getByRole('checkbox').check();
  await page.getByRole('button', { name: 'Start sharing' }).click();
  await expect(page.getByRole('alert')).toContainText('desktop app');
  await page.getByRole('button', { name: 'Dismiss error' }).click();
  await expect(page.getByRole('img', { name: 'Spacie' })).toBeVisible();
  await page.screenshot({ path: 'artifacts/host-desktop.png', fullPage: true });
  await page.getByRole('button', { name: 'Connect to a device', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'A terminal, wherever they are.' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Open shell' })).toBeDisabled();
  await page.screenshot({ path: 'artifacts/operator-desktop.png', fullPage: true });
  await page.getByRole('button', { name: 'Agent access', exact: true }).click();
  await expect(page.getByText('node agent/tether.mjs exec', { exact: false })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Copy configuration' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Copy agent prompt' })).toBeVisible();
  await expect(page.getByLabel('Copy-paste agent prompt')).toContainText('tether_pair_status');
  await page.screenshot({ path: 'artifacts/agents-desktop.png', fullPage: true });
  await page.getByRole('button', { name: 'Activity', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'No activity yet' })).toBeVisible();
  expect(errors).toEqual([]);
});
test('compact workspace does not overflow horizontally', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Bring debugging closer.' })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
    true,
  );
  await page.screenshot({ path: 'artifacts/host-mobile.png', fullPage: true });
});
