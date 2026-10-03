import { readdir, readFile, mkdir, copyFile, writeFile, rm } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { basename, dirname, join, resolve } from 'node:path';
const platform = process.argv[2];
if (!['windows-x64', 'macos-arm64', 'macos-x64', 'linux-x64'].includes(platform))
  throw new Error('Choose a supported installer platform.');
const source = resolve('src-tauri/target/release/bundle');
const outputRoot = resolve('artifacts/installers');
const destination = resolve(outputRoot, platform);
if (dirname(destination) !== outputRoot)
  throw new Error('Artifact output must remain inside its generated directory.');
await rm(destination, { recursive: true, force: true });
await mkdir(destination, { recursive: true });
const version = JSON.parse(await readFile('package.json', 'utf8')).version;
async function files(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory() && !entry.name.endsWith('.app')) result.push(...(await files(path)));
    else if (entry.isFile() && /\.(exe|dmg|deb|AppImage|app\.tar\.gz)(\.sig)?$/.test(entry.name))
      result.push(path);
  }
  return result.sort();
}
const installers = (await files(source)).filter(
  (path) =>
    basename(path).startsWith(`Tether_${version}_`) ||
    (platform.startsWith('macos') && /^Tether\.app\.tar\.gz(\.sig)?$/.test(basename(path))),
);
const required = platform.startsWith('windows')
  ? [/\.exe$/, /\.exe\.sig$/]
  : platform.startsWith('macos')
    ? [/\.dmg$/, /\.app\.tar\.gz$/, /\.app\.tar\.gz\.sig$/]
    : [/\.deb$/, /\.deb\.sig$/, /\.AppImage$/, /\.AppImage\.sig$/];
for (const extension of required)
  if (!installers.some((file) => extension.test(file)))
    throw new Error(`Missing expected installer: ${extension}`);
const names = new Set();
const checksums = [];
for (const path of installers) {
  // Tauri gives both macOS architectures the same archive name. Keep release assets distinct.
  const architecture = platform === 'macos-arm64' ? 'aarch64' : 'x64';
  const name =
    platform.startsWith('macos') && /\.app\.tar\.gz(\.sig)?$/.test(path)
      ? `Tether_${version}_${architecture}.app.tar.gz${path.endsWith('.sig') ? '.sig' : ''}`
      : basename(path);
  if (names.has(name)) throw new Error(`Installer filename collision: ${name}`);
  names.add(name);
  await copyFile(path, join(destination, name));
  const hash = createHash('sha256')
    .update(await readFile(path))
    .digest('hex');
  checksums.push(`${hash}  ${name}`);
}
await writeFile(join(destination, `SHA256SUMS-${platform}.txt`), checksums.join('\n') + '\n');
console.log(`Collected ${installers.length} ${platform} installer(s) and SHA-256 checksums.`);
