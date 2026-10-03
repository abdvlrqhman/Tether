import { readFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const json = async (path) => JSON.parse(await readFile(new URL(path, import.meta.url), 'utf8'));
const pkg = await json('../package.json');
const lock = await json('../package-lock.json');
const tauri = await json('../src-tauri/tauri.conf.json');
const cargo = await readFile(new URL('../src-tauri/Cargo.toml', import.meta.url), 'utf8');
const cargoVersion = cargo
  .split('[package]')[1]
  ?.split(/^\[/m)[0]
  ?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
assert.match(pkg.version, /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/);
for (const [name, version] of Object.entries({
  'package-lock': lock.version,
  'package-lock root': lock.packages[''].version,
  Tauri: tauri.version,
  Cargo: cargoVersion,
}))
  assert.equal(version, pkg.version, `${name} version must match package.json`);
if (process.env.TETHER_RELEASE_TAG)
  assert.equal(
    process.env.TETHER_RELEASE_TAG,
    `v${pkg.version}`,
    'Release tag must match the app version',
  );
console.log(
  `Tether version ${pkg.version} is consistent${process.env.TETHER_RELEASE_TAG ? ' with its release tag' : ''}.`,
);
