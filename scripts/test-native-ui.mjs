// Exercise the real Windows Tauri/WebView2 IPC boundary, not the browser preview.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { promises as fs } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createServer } from 'node:net';
import assert from 'node:assert/strict';

if (process.platform !== 'win32')
  throw new Error('This native UI check requires Windows WebView2.');
const binary = resolve(process.argv[2] || 'src-tauri/target/release/tether.exe');
await fs.access(binary);
const profile = await fs.mkdtemp(join(tmpdir(), 'tether-webview-test-'));
const reservation = createServer();
await new Promise((resolve) => reservation.listen(0, '127.0.0.1', resolve));
const port = reservation.address().port;
await new Promise((resolve) => reservation.close(resolve));
const child = spawn(binary, [], {
  windowsHide: true,
  stdio: 'ignore',
  env: {
    ...process.env,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
    WEBVIEW2_USER_DATA_FOLDER: profile,
  },
});
let browser, page, base;
const deadline = Date.now() + 90000;
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function request(path, method = 'GET', body, token) {
  const response = await fetch(`${base}${path}`, {
    method,
    headers: {
      ...(body ? { 'content-type': 'application/json' } : {}),
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
    signal: AbortSignal.timeout(5000),
    redirect: 'error',
  });
  return { status: response.status, body: await response.json() };
}
try {
  while (Date.now() < deadline) {
    if (child.exitCode !== null) throw new Error(`Desktop exited with ${child.exitCode}`);
    try {
      browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 });
      break;
    } catch {
      await delay(250);
    }
  }
  assert.ok(browser, 'WebView2 debugging endpoint did not become ready');
  page = browser.contexts()[0].pages()[0];
  await page.getByRole('heading', { name: 'Bring debugging closer.' }).waitFor();
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  assert.equal(await page.getByText('Browser preview', { exact: false }).count(), 0);
  await expect(page.getByLabel('Starting folder')).not.toHaveValue('', { timeout: 10000 });
  await fs.mkdir('artifacts', { recursive: true });
  await page.screenshot({ path: 'artifacts/native-desktop.png' });
  await page.getByText('Local only', { exact: true }).click();
  await page.getByRole('checkbox').check();
  await page.getByRole('button', { name: 'Start sharing', exact: true }).click();
  await page.getByLabel('Invitation JSON').waitFor();
  const invitation = JSON.parse(await page.getByLabel('Invitation JSON').inputValue());
  base = invitation.url;
  assert.equal((await request('/health')).body.service, 'tether');
  const receipt = await request('/v1/pair', 'POST', {
    invite: invitation.invite,
    operator: 'Native UI verification',
  });
  assert.equal(receipt.status, 200);
  assert.equal(
    (await request(`/v1/pair/${receipt.body.id}`, 'GET', undefined, receipt.body.proof)).body
      .status,
    'pending',
  );
  await page.getByRole('button', { name: 'Approve access', exact: true }).click();
  await page.getByRole('button', { name: 'End access', exact: true }).waitFor();
  const approved = await request(
    `/v1/pair/${receipt.body.id}`,
    'GET',
    undefined,
    receipt.body.proof,
  );
  const token = approved.body.token;
  assert.equal(approved.body.status, 'approved');
  const submitted = await request(
    '/v1/exec',
    'POST',
    {
      command: "Write-Output 'native-desktop-ok'; exit 7",
      timeout_seconds: 15,
    },
    token,
  );
  assert.equal(submitted.status, 200);
  let result;
  while (Date.now() < deadline) {
    result = (await request(`/v1/jobs/${submitted.body.id}`, 'GET', undefined, token)).body;
    if (result.status === 'finished') break;
    await delay(100);
  }
  assert.equal(result.result.exit_code, 7);
  assert.match(result.result.stdout, /native-desktop-ok/);
  await page.getByRole('button', { name: 'End access', exact: true }).click();
  assert.equal((await request('/v1/session', 'GET', undefined, token)).status, 401);
  await page.getByRole('button', { name: 'Stop sharing', exact: true }).click();
  await page.getByRole('button', { name: 'Start sharing', exact: true }).waitFor();
  await assert.rejects(() => request('/health'));
  base = undefined;
  await page.getByRole('button', { name: 'Agent access', exact: true }).click();
  assert.ok((await page.locator('pre').first().textContent()).includes('tether.exe'));
  const scrollBefore = await page.evaluate(() => window.scrollY);
  await delay(2500); // Observe multiple background overview polls without interacting.
  assert.equal(
    await page.evaluate(() => window.scrollY),
    scrollBefore,
    'Background polling moved the page',
  );
  await page.getByRole('button', { name: 'Copy agent prompt', exact: true }).click();
  await page.getByRole('button', { name: 'Copied prompt', exact: true }).waitFor();
  assert.deepEqual(errors, []);
  console.log(
    'Native desktop passed: IPC, host consent, local approval, real command, revocation, shutdown, and MCP prompt clipboard.',
  );
} finally {
  if (page && base) {
    await page
      .getByRole('button', { name: 'Stop sharing', exact: true })
      .click()
      .catch(() => {});
  }
  await browser?.close();
  if (child.exitCode === null) child.kill();
  // WebView2 may briefly retain its files after the parent exits.
  await fs.rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
}
