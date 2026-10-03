import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { verifyReleaseArtifacts, createUpdateManifest } from '../scripts/release-artifacts.mjs';
async function fixture() {
  const dir = await mkdtemp(join(tmpdir(), 'tether-release-test-'));
  for (const [platform, files] of Object.entries({
    'windows-x64': ['Tether_0.1.0_x64-setup.exe', 'Tether_0.1.0_x64-setup.exe.sig'],
    'macos-arm64': [
      'Tether_0.1.0_aarch64.dmg',
      'Tether_0.1.0_aarch64.app.tar.gz',
      'Tether_0.1.0_aarch64.app.tar.gz.sig',
    ],
    'macos-x64': [
      'Tether_0.1.0_x64.dmg',
      'Tether_0.1.0_x64.app.tar.gz',
      'Tether_0.1.0_x64.app.tar.gz.sig',
    ],
    'linux-x64': [
      'Tether_0.1.0_amd64.deb',
      'Tether_0.1.0_amd64.AppImage',
      'Tether_0.1.0_amd64.AppImage.sig',
    ],
  })) {
    const lines = [];
    for (const name of files) {
      const contents = name.endsWith('.sig')
        ? Buffer.from('test-signature-fixture'.repeat(8)).toString('base64')
        : 'installer-fixture';
      await writeFile(join(dir, name), contents);
      lines.push(`${createHash('sha256').update(contents).digest('hex')}  ${name}`);
    }
    await writeFile(join(dir, `SHA256SUMS-${platform}.txt`), lines.join('\n') + '\n');
  }
  return dir;
}
test('release requires every native installer with matching checksums', async () => {
  const dir = await fixture();
  try {
    assert.equal((await verifyReleaseArtifacts(dir)).length, 15);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('updater metadata maps every architecture to its own signed package', async () => {
  const dir = await fixture();
  try {
    const manifest = await createUpdateManifest(dir, {
      tag: 'v0.1.0',
      repository: 'abdvlrqhman/Tether',
      notes: 'Release notes',
      publishedAt: '2026-10-03T00:00:00Z',
    });
    assert.equal(manifest.version, '0.1.0');
    assert.deepEqual(Object.keys(manifest.platforms).sort(), [
      'darwin-aarch64',
      'darwin-x86_64',
      'linux-x86_64',
      'windows-x86_64',
    ]);
    assert.match(
      manifest.platforms['darwin-aarch64'].url,
      /v0\.1\.0\/Tether_0\.1\.0_aarch64\.app\.tar\.gz$/,
    );
    assert.match(
      manifest.platforms['darwin-x86_64'].url,
      /v0\.1\.0\/Tether_0\.1\.0_x64\.app\.tar\.gz$/,
    );
    assert.match(manifest.platforms['linux-x86_64'].url, /\.AppImage$/);
    assert.match(manifest.platforms['windows-x86_64'].url, /\.exe$/);
    assert.ok(manifest.platforms['windows-x86_64'].signature.length > 100);
    assert.equal((await verifyReleaseArtifacts(dir)).length, 16);
    await rm(join(dir, 'Tether_0.1.0_x64-setup.exe.sig'));
    await assert.rejects(
      () =>
        createUpdateManifest(dir, { tag: 'v0.1.0', repository: 'abdvlrqhman/Tether', notes: '' }),
      /Missing artifact/,
    );
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
test('release refuses corrupted binaries and missing platforms', async () => {
  const dir = await fixture();
  try {
    await writeFile(join(dir, 'Tether_0.1.0_x64-setup.exe'), 'corrupted');
    await assert.rejects(() => verifyReleaseArtifacts(dir), /Checksum mismatch/);
    await rm(join(dir, 'SHA256SUMS-windows-x64.txt'));
    await assert.rejects(() => verifyReleaseArtifacts(dir), /Missing checksums/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
