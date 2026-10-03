import { readFile, readdir, writeFile } from 'node:fs/promises';
import { basename, join } from 'node:path';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';

export const platforms = {
  'windows-x64': {
    target: 'windows-x86_64',
    extensions: [/x64.*\.exe$/, /x64.*\.exe\.sig$/],
    update: /\.exe$/,
  },
  'macos-arm64': {
    target: 'darwin-aarch64',
    extensions: [/aarch64\.dmg$/, /aarch64\.app\.tar\.gz$/, /aarch64\.app\.tar\.gz\.sig$/],
    update: /aarch64\.app\.tar\.gz$/,
  },
  'macos-x64': {
    target: 'darwin-x86_64',
    extensions: [/x64\.dmg$/, /x64\.app\.tar\.gz$/, /x64\.app\.tar\.gz\.sig$/],
    update: /x64\.app\.tar\.gz$/,
  },
  'linux-x64': {
    target: 'linux-x86_64',
    extensions: [/amd64\.deb$/, /amd64\.AppImage$/, /amd64\.AppImage\.sig$/],
    update: /\.AppImage$/,
  },
};
export async function verifyReleaseArtifacts(directory) {
  const names = (await readdir(directory)).sort(),
    checksums = new Map();
  for (const [platform, { extensions }] of Object.entries(platforms)) {
    const name = `SHA256SUMS-${platform}.txt`;
    assert.ok(names.includes(name), `Missing checksums for ${platform}`);
    const entries = (await readFile(join(directory, name), 'utf8')).trim().split('\n');
    const platformNames = [];
    for (const line of entries) {
      const match = /^([a-f0-9]{64})  (.+)$/.exec(line.trim());
      assert.ok(match, `Invalid checksum entry in ${name}`);
      const [, hash, file] = match;
      assert.equal(basename(file), file, 'Checksum paths must be filenames');
      assert.ok(
        !file.includes('/') && !file.includes('\\'),
        'Checksum paths cannot leave the artifact directory',
      );
      assert.ok(names.includes(file), `Missing artifact ${file}`);
      assert.ok(!checksums.has(file), `Duplicate artifact ${file}`);
      assert.equal(
        createHash('sha256')
          .update(await readFile(join(directory, file)))
          .digest('hex'),
        hash,
        `Checksum mismatch: ${file}`,
      );
      checksums.set(file, hash);
      platformNames.push(file);
    }
    assert.equal(
      platformNames.length,
      extensions.length,
      `Unexpected artifact count for ${platform}`,
    );
    for (const extension of extensions)
      assert.equal(
        platformNames.filter((file) => extension.test(file)).length,
        1,
        `Missing artifact type for ${platform}`,
      );
  }
  for (const name of checksums.keys())
    if (name.endsWith('.sig')) {
      assert.ok(checksums.has(name.slice(0, -4)), `Signature has no matching package: ${name}`);
      const signature = (await readFile(join(directory, name), 'utf8')).trim();
      assert.ok(
        /^[A-Za-z0-9+/]+={0,2}$/.test(signature) && signature.length > 100,
        `Invalid signature encoding: ${name}`,
      );
    }
  assert.equal(
    names.length,
    checksums.size + 4 + Number(names.includes('latest.json')),
    'Release contains unexpected or incomplete artifacts',
  );
  return names.map((name) => join(directory, name));
}
export async function createUpdateManifest(
  directory,
  { tag, repository, notes, publishedAt = new Date().toISOString() },
) {
  assert.match(tag, /^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/, 'Invalid release tag');
  assert.match(repository, /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/, 'Invalid repository');
  const assets = await verifyReleaseArtifacts(directory);
  const manifest = { version: tag.slice(1), notes, pub_date: publishedAt, platforms: {} };
  for (const { target, update } of Object.values(platforms)) {
    const asset = assets.find((path) => update.test(path));
    assert.ok(asset, `Missing updater package for ${target}`);
    manifest.platforms[target] = {
      signature: (await readFile(`${asset}.sig`, 'utf8')).trim(),
      url: `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(basename(asset))}`,
    };
  }
  await writeFile(join(directory, 'latest.json'), JSON.stringify(manifest, null, 2) + '\n');
  return manifest;
}
