import test from 'node:test';
import assert from 'node:assert/strict';
import { promises as fs } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { validateURL, saveSession, loadSession, request } from '../agent/client.mjs';
import { createServer } from 'node:http';
test('operator endpoints enforce HTTPS except loopback', () => {
  assert.equal(validateURL('http://127.0.0.1:9876'), 'http://127.0.0.1:9876');
  assert.equal(validateURL('https://debug.trycloudflare.com/'), 'https://debug.trycloudflare.com');
  for (const url of [
    'http://example.com',
    'file:///tmp/x',
    'https://user:password@example.com',
    'https://example.com/path',
    'https://example.com/?token=x',
    'https://example.com/#secret',
  ])
    assert.throws(() => validateURL(url));
});
test('private credentials round trip and expired sessions are refused', async () => {
  const directory = await fs.mkdtemp(join(tmpdir(), 'tether-test-'));
  const path = join(directory, 'agent.session.json');
  try {
    const session = {
      url: 'https://test.trycloudflare.com',
      token: 'x'.repeat(43),
      expires_at: Date.now() / 1000 + 60,
    };
    await saveSession(path, session);
    assert.deepEqual(await loadSession(path), session);
    await saveSession(path, { ...session, expires_at: 1 });
    await assert.rejects(loadSession(path), /expired/);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});
test('transport never follows a redirect with bearer credentials', async () => {
  const server = createServer((req, res) => {
    res.writeHead(302, { location: 'http://127.0.0.1:1/stolen' });
    res.end();
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    await assert.rejects(
      request(`http://127.0.0.1:${server.address().port}`, '/v1/session', { token: 'secret' }),
    );
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});
