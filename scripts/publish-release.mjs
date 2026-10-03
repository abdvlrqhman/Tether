import { spawnSync } from 'node:child_process';
import { access, readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { verifyReleaseArtifacts, createUpdateManifest } from './release-artifacts.mjs';
const tag = process.env.RELEASE_TAG;
if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag || '')) throw new Error('Invalid release tag');
const versionNotes = `docs/releases/${tag.slice(1)}.md`;
const notes = await access(versionNotes)
  .then(() => versionNotes)
  .catch(() => '.github/RELEASE_NOTES.md');
await createUpdateManifest(resolve('artifacts/release'), {
  tag,
  repository: process.env.GH_REPO || 'abdvlrqhman/Tether',
  notes: await readFile(notes, 'utf8'),
});
const assets = await verifyReleaseArtifacts(resolve('artifacts/release'));
if (process.argv.includes('--verify-only')) {
  console.log(`Verified ${assets.length} release assets for ${tag}.`);
  process.exit(0);
}
function gh(args) {
  const result = spawnSync('gh', args, { encoding: 'utf8', windowsHide: true });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(result.stderr || `gh ${args[0]} failed`);
  return result.stdout.trim();
}
const existing = spawnSync('gh', ['release', 'view', tag, '--json', 'isDraft,url'], {
  encoding: 'utf8',
  windowsHide: true,
});
let release;
if (existing.status === 0) release = JSON.parse(existing.stdout);
else if (!/release not found|Not Found/i.test(existing.stderr || ''))
  throw new Error(existing.stderr || 'Cannot inspect release');
if (release && !release.isDraft) {
  console.log(`Already published; assets left unchanged: ${release.url}`);
  process.exit(0);
}
const prerelease = tag.includes('-');
if (!release)
  gh([
    'release',
    'create',
    tag,
    '--verify-tag',
    '--draft',
    '--title',
    `Tether ${tag}`,
    '--notes-file',
    notes,
    ...(prerelease ? ['--prerelease'] : []),
  ]);
// An interrupted draft can be resumed, but published assets are never overwritten.
gh(['release', 'upload', tag, ...assets, '--clobber']);
const uploaded = JSON.parse(gh(['release', 'view', tag, '--json', 'assets'])).assets;
if (uploaded.length !== assets.length)
  throw new Error('Draft asset count does not match verified artifacts');
const expectedNames = assets.map((path) => path.split(/[\\/]/).pop());
for (const asset of uploaded)
  if (!expectedNames.includes(asset.name)) throw new Error(`Unexpected draft asset: ${asset.name}`);
gh([
  'release',
  'edit',
  tag,
  '--draft=false',
  '--notes-file',
  notes,
  prerelease ? '--latest=false' : '--latest',
]);
console.log(gh(['release', 'view', tag, '--json', 'url', '--jq', '.url']));
