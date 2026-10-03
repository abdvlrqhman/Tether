import { test, expect } from '@playwright/test';
test('host workspace, operator flow, agent docs, and branding render', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Share your workspace' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Start sharing' })).toBeDisabled();
  await page.getByLabel('Starting folder').fill('D:\\Projects');
  await page.getByRole('checkbox').check();
  await page.getByRole('button', { name: 'Start sharing' }).click();
  await expect(page.getByRole('alert')).toContainText('desktop app');
  await page.getByRole('button', { name: 'Dismiss error' }).click();
  await expect(page.getByRole('img', { name: 'Spacie' })).toBeVisible();
  await page.screenshot({ path: 'artifacts/host-desktop.png', fullPage: true });
  await page.getByRole('button', { name: 'Connect to a device', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Connect to a workspace' })).toBeVisible();
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
  await expect(page.getByRole('heading', { name: 'Share your workspace' })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
    true,
  );
  await page.screenshot({ path: 'artifacts/host-mobile.png', fullPage: true });
});

// Replace only the native boundary. The real hook, plugin resource, progress channel,
// Settings screen, and install/restart controls execute together.
async function mockNative(
  page: import('@playwright/test').Page,
  mode: 'current' | 'available' | 'sharing' | 'offline',
) {
  await page.addInitScript((mode) => {
    const bridge = window as unknown as {
      isTauri: boolean;
      __TAURI_INTERNALS__: unknown;
      __updates: { mode: string; calls: string[]; finish?: () => void };
    };
    bridge.isTauri = true;
    bridge.__updates = { mode, calls: [] };
    bridge.__TAURI_INTERNALS__ = {
      transformCallback: () => 1,
      unregisterCallback: () => {},
      invoke: async (
        name: string,
        args: { onEvent?: { onmessage: (e: unknown) => void } } = {},
      ) => {
        bridge.__updates.calls.push(name);
        if (name === 'overview')
          return {
            home: '/projects',
            executable: '/tether',
            audit_path: '/audit.jsonl',
            host: {
              running: mode === 'sharing',
              tunnel: 'local',
              url: 'http://127.0.0.1:1234',
              working_directory: '/projects',
              invite: null,
              pending: [],
              session: null,
              audit: [],
              error: null,
            },
            remote: { status: '', error: null },
          };
        if (name === 'plugin:updater|check') {
          if (bridge.__updates.mode === 'offline') throw new Error('Network unavailable');
          if (bridge.__updates.mode === 'current') return null;
          return {
            rid: 4,
            currentVersion: '0.1.0',
            version: '0.2.0',
            body: 'Improved terminal stability.',
            rawJson: {},
          };
        }
        if (name === 'plugin:updater|download') {
          args.onEvent?.onmessage({ event: 'Started', data: { contentLength: 100 } });
          args.onEvent?.onmessage({ event: 'Progress', data: { chunkLength: 60 } });
          return new Promise((resolve) => {
            bridge.__updates.finish = () => {
              args.onEvent?.onmessage({ event: 'Finished' });
              resolve(5);
            };
          });
        }
        return 1;
      },
    };
  }, mode);
}
test('update checks report current versions and recover from network failure', async ({ page }) => {
  await mockNative(page, 'offline');
  await page.goto('/');
  await page.getByRole('button', { name: 'Software updates', exact: true }).click();
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Network unavailable');
  await page.evaluate(() => {
    (window as unknown as { __updates: { mode: string } }).__updates.mode = 'current';
  });
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('You’re up to date');
  await expect(page.getByRole('alert')).toHaveCount(0);
});
test('active sharing blocks update installation but allows checking release notes', async ({
  page,
}) => {
  await mockNative(page, 'sharing');
  await page.goto('/');
  await page.getByRole('button', { name: 'Software updates', exact: true }).click();
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Tether 0.2.0 is available');
  await expect(page.getByText('Improved terminal stability.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Install update', exact: true })).toBeDisabled();
  await expect(
    page.getByText('Stop sharing and disconnect from devices before installing an update.'),
  ).toBeVisible();
});
test('updates expose download progress, validate idle state, install, and restart', async ({
  page,
}) => {
  await mockNative(page, 'available');
  await page.goto('/');
  await page.getByRole('button', { name: 'Software updates', exact: true }).click();
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await page.getByRole('button', { name: 'Install update', exact: true }).click();
  await expect(page.getByRole('progressbar', { name: 'Update download progress' })).toHaveAttribute(
    'value',
    '60',
  );
  await expect(page.getByRole('button', { name: 'Updating…', exact: true })).toBeDisabled();
  await page.evaluate(() => {
    (window as unknown as { __updates: { finish: () => void } }).__updates.finish();
  });
  await expect(
    page.getByText('Update installed. Restart to finish.', { exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Restart Tether', exact: true }).click();
  const calls = await page.evaluate(
    () => (window as unknown as { __updates: { calls: string[] } }).__updates.calls,
  );
  expect(
    calls.filter((name) =>
      [
        'plugin:updater|download',
        'begin_update',
        'plugin:updater|install',
        'plugin:process|restart',
      ].includes(name),
    ),
  ).toEqual([
    'plugin:updater|download',
    'begin_update',
    'plugin:updater|install',
    'plugin:process|restart',
  ]);
});
