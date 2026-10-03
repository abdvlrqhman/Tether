import { readdir, readFile, mkdir, copyFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { basename, join, resolve } from 'node:path';
const platform = process.argv[2];
if (!['windows-x64', 'macos-arm64', 'macos-x64', 'linux-x64'].includes(platform))
  throw new Error('Choose a supported installer platform.');
const source = resolve('src-tauri/target/release/bundle');
const destination = resolve('artifacts/installers', platform);
await mkdir(destination, { recursive: true });
async function files(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory() && !entry.name.endsWith('.app')) result.push(...(await files(path)));
    else if (entry.isFile() && /\.(exe|dmg|deb|AppImage)$/.test(entry.name)) result.push(path);
  }
  return result.sort();
}
const installers = await files(source);
const required = platform.startsWith('windows')
  ? [/\.exe$/]
  : platform.startsWith('macos')
    ? [/\.dmg$/]
    : [/\.deb$/, /\.AppImage$/];
for (const extension of required)
  if (!installers.some((file) => extension.test(file)))
    throw new Error(`Missing expected installer: ${extension}`);
const names = new Set();
const checksums = [];
for (const path of installers) {
  const name = basename(path);
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
