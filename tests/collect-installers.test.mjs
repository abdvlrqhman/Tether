import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, readdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFileSync } from 'node:child_process';

test('Linux collection includes both signatures and excludes cached older releases', async () => {
  const root = await mkdtemp(join(tmpdir(), 'tether-collect-test-'));
  const script = resolve('scripts/collect-installers.mjs');
  const source = join(root, 'src-tauri/target/release/bundle');
  const output = join(root, 'artifacts/installers/linux-x64');
  const extensions = ['deb', 'deb.sig', 'AppImage', 'AppImage.sig'];
  try {
    await mkdir(source, { recursive: true });
    for (const version of ['0.1.0', '0.1.1', '0.1.2']) {
      for (const ext of extensions)
        await writeFile(join(source, `Tether_${version}_amd64.${ext}`), 'fixture');
    }
    for (const version of ['0.1.1', '0.1.2']) {
      await writeFile(join(root, 'package.json'), JSON.stringify({ version }));
      execFileSync(process.execPath, [script, 'linux-x64'], { cwd: root, windowsHide: true });
      assert.deepEqual(
        (await readdir(output)).sort(),
        [
          'SHA256SUMS-linux-x64.txt',
          ...extensions.map((ext) => `Tether_${version}_amd64.${ext}`),
        ].sort(),
      );
      assert.equal(
        (await readFile(join(output, 'SHA256SUMS-linux-x64.txt'), 'utf8')).trim().split('\n')
          .length,
        4,
      );
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
