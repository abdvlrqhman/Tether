// Download only a pinned Cloudflare release, verify SHA-256, then prepare a Tauri sidecar.
import { promises as fs } from 'node:fs';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';
const root = dirname(dirname(fileURLToPath(import.meta.url)));
const lock = JSON.parse(await fs.readFile(join(root, 'scripts/cloudflared-lock.json'), 'utf8'));
const target =
  process.env.TAURI_ENV_TARGET_TRIPLE ||
  execFileSync('rustc', ['--print', 'host-tuple'], { encoding: 'utf8', windowsHide: true }).trim();
const asset = lock.assets[target];
if (!asset) throw new Error(`No verified cloudflared asset configured for ${target}.`);
const suffix = target.includes('windows') ? '.exe' : '';
const destination = join(root, 'src-tauri/binaries', `cloudflared-${target}${suffix}`);
const stamp = destination + '.verified.json';
if (!asset.name.endsWith('.tgz')) {
  const candidate = await fs.readFile(destination).catch(() => null);
  if (candidate && createHash('sha256').update(candidate).digest('hex') === asset.sha256) {
    await fs.writeFile(
      stamp,
      JSON.stringify({
        version: lock.version,
        asset_sha256: asset.sha256,
        binary_sha256: asset.sha256,
      }),
    );
    console.log(`Verified cloudflared ${lock.version} already present for ${target}.`);
    process.exit(0);
  }
}
let verified;
try {
  verified = JSON.parse(await fs.readFile(stamp, 'utf8'));
} catch {}
if (verified?.version === lock.version && verified?.asset_sha256 === asset.sha256) {
  const bytes = await fs.readFile(destination).catch(() => null);
  if (bytes && createHash('sha256').update(bytes).digest('hex') === verified.binary_sha256) {
    console.log(`Verified cloudflared ${lock.version} already prepared for ${target}.`);
    process.exit(0);
  }
}
const url = `https://github.com/cloudflare/cloudflared/releases/download/${lock.version}/${asset.name}`;
console.log(`Preparing cloudflared ${lock.version} for ${target}…`);
const response = await fetch(url, { signal: AbortSignal.timeout(600000) });
if (!response.ok) throw new Error(`cloudflared download failed: ${response.status}`);
const chunks = [];
let size = 0;
for await (const chunk of response.body) {
  size += chunk.length;
  if (size > 100 * 1024 * 1024) throw new Error('Sidecar download exceeded 100 MB.');
  chunks.push(chunk);
}
const bytes = Buffer.concat(chunks);
if (createHash('sha256').update(bytes).digest('hex') !== asset.sha256)
  throw new Error('cloudflared checksum mismatch. Refusing to prepare sidecar.');
await fs.mkdir(dirname(destination), { recursive: true });
let binary = bytes;
if (asset.name.endsWith('.tgz')) {
  const temp = await fs.mkdtemp(join(tmpdir(), 'tether-sidecar-'));
  try {
    const archive = join(temp, 'cloudflared.tgz');
    await fs.writeFile(archive, bytes);
    execFileSync('tar', ['-xzf', archive, '-C', temp, 'cloudflared'], { stdio: 'pipe' });
    binary = await fs.readFile(join(temp, 'cloudflared'));
  } finally {
    await fs.rm(temp, { recursive: true, force: true });
  }
}
await fs.writeFile(destination, binary, { mode: 0o755 });
await fs.chmod(destination, 0o755);
await fs.writeFile(
  stamp,
  JSON.stringify({
    version: lock.version,
    asset_sha256: asset.sha256,
    binary_sha256: createHash('sha256').update(binary).digest('hex'),
  }),
);
console.log(`Prepared and verified ${destination}`);
