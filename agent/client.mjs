// Transport and private credential storage, independent of CLI argument handling.
import { promises as fs } from 'node:fs';
import { dirname, join } from 'node:path';
import { homedir } from 'node:os';
import { execFileSync } from 'node:child_process';
export function validateURL(input) {
  const url = new URL(input);
  const local = ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname);
  if (
    !(url.protocol === 'https:' || (url.protocol === 'http:' && local)) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    url.pathname !== '/'
  )
    throw new Error(
      'Use HTTPS, or loopback HTTP, without credentials, a path, query, or fragment.',
    );
  return url.origin;
}
export const defaultSession = join(
  process.env.LOCALAPPDATA || join(homedir(), '.config'),
  'spacie-tether',
  'agent.session.json',
);
export async function request(base, path, { token, body, method = 'GET' } = {}) {
  const response = await fetch(validateURL(base) + path, {
    method,
    redirect: 'error',
    signal: AbortSignal.timeout(15000),
    headers: {
      ...(token ? { authorization: `Bearer ${token}` } : {}),
      ...(body ? { 'content-type': 'application/json' } : {}),
    },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
  let size = 0;
  const chunks = [];
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > 3 * 1024 * 1024) throw new Error('Host response exceeded size limit.');
    chunks.push(chunk);
  }
  const value = JSON.parse(Buffer.concat(chunks).toString('utf8'));
  if (!response.ok) throw new Error(value.error || `Host returned HTTP ${response.status}`);
  return value;
}
export async function saveSession(path, session) {
  await fs.mkdir(dirname(path), { recursive: true, mode: 0o700 });
  const temp = `${path}.${process.pid}.tmp`;
  try {
    await fs.writeFile(temp, JSON.stringify(session), { mode: 0o600, flag: 'wx' });
    if (process.platform === 'win32') {
      const identity = execFileSync('whoami.exe', ['/user', '/fo', 'csv', '/nh'], {
        encoding: 'utf8',
        windowsHide: true,
      });
      const sid = identity.match(/S-1-[0-9-]+/)?.[0];
      if (!sid) throw new Error('Unable to determine current user SID.');
      execFileSync('icacls.exe', [temp, '/inheritance:r', '/grant:r', `*${sid}:(F)`], {
        stdio: 'pipe',
        windowsHide: true,
      });
    }
    await fs.rename(temp, path);
  } catch (error) {
    await fs.rm(temp, { force: true });
    throw error;
  }
}
export async function loadSession(path) {
  const stat = await fs.stat(path);
  if (process.platform !== 'win32' && (stat.mode & 0o077) !== 0)
    throw new Error('Session file is accessible to other users. Run chmod 600 on it.');
  const data = JSON.parse(await fs.readFile(path, 'utf8'));
  validateURL(data.url);
  if (
    typeof data.token !== 'string' ||
    data.token.length !== 43 ||
    !Number.isFinite(data.expires_at) ||
    Date.now() / 1000 >= data.expires_at
  )
    throw new Error('Session is invalid or expired. Pair again.');
  return data;
}
