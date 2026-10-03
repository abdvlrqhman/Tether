# Development and release workflows

One reusable pipeline validates pull requests, installer builds, and tagged releases on Windows x64, macOS arm64/x64, and Linux x64. Ordinary checks require no signing secrets. Packaging creates signed updater artifacts.

## Daily development

1. Create a branch from `main`.
2. Run version/format/build checks, Rust formatting/Clippy, and relevant tests.
3. Open a pull request. **Required checks** passes only when every platform succeeds.
4. Merge after checks and resolved conversations. Main blocks force pushes and deletion, requires an up-to-date branch, and enforces these rules for administrators. This solo repository currently requires zero additional reviewer approvals.

Actions are pinned by SHA. Checkouts do not persist credentials, jobs have timeouts, and verification uses read-only repository access. Pull requests never package with release secrets. UI failure diagnostics last seven days; installer artifacts last fourteen days.

Dependabot groups frontend, Rust, and GitHub Actions updates weekly. Major upgrades still require migration and verification; they are not merged automatically. Cloudflared has a separate version/digest lockfile in `scripts/cloudflared-lock.json`.

## Build installers without releasing

```sh
gh workflow run installers.yml --ref main
```

Each platform completes checks and tests its packaged MCP executable before uploading installers, updater packages/signatures, and `SHA256SUMS-<platform>.txt`:

| Platform            | Installer           | Updater package                                   |
| ------------------- | ------------------- | ------------------------------------------------- |
| Windows x64         | NSIS `.exe`         | The same `.exe` plus `.sig`                       |
| macOS Apple Silicon | `.dmg`              | `Tether_<version>_aarch64.app.tar.gz` plus `.sig` |
| macOS Intel         | `.dmg`              | `Tether_<version>_x64.app.tar.gz` plus `.sig`     |
| Linux x64           | `.deb`, `.AppImage` | The `.AppImage` plus `.sig`                       |

macOS updater archives are renamed during collection to avoid architecture filename collisions. Checksums cover installers, update archives, and signature files. Manual installer builds publish no release or update manifest.

For local packaging without release signatures:

```sh
npm run tauri build -- --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

In PowerShell, write the override JSON to a temporary config file and pass its path if your shell changes the quoting. Development with `npm run tauri dev` does not require a signing key.

## Updater signing key

The app's public key and stable endpoint live in `src-tauri/tauri.conf.json`. **`TAURI_SIGNING_PRIVATE_KEY`** is configured as a repository secret and passed only to the packaging step in trusted installer/release runs. The initial key has an empty password, set explicitly through `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` in the workflow.

The original private key is stored outside this checkout at `%LOCALAPPDATA%\spacie-tether-release\updater.key`, with access limited to the current Windows user. Keep an encrypted, restricted backup. Never commit or log it. Losing the key prevents existing installations from trusting future updates; replacing the bundled public key alone does not migrate installed clients.

For a separate fork, generate your own key with `npm run tauri -- signer generate --write-keys <private-path>`, configure your own secret/public key, and change the GitHub endpoint. Do not reuse Spacie's release identity.

Updater signatures authenticate packages within Tether. Windows publisher signing and Apple Developer ID signing/notarization are separate and are not configured for the initial installers. Their setup is described in Tauri's [Windows signing](https://v2.tauri.app/distribute/sign/windows/) and [macOS signing](https://v2.tauri.app/distribute/sign/macos/) guides. Linux in-app installation requires AppImage; `.deb` users install the new Debian package.

## Publish a release

Align `package.json`, root entries in `package-lock.json`, `src-tauri/Cargo.toml`, Tether's entry in `Cargo.lock`, and `src-tauri/tauri.conf.json`. Run `npm run check:version`; Cargo's `--locked` checks verify the Rust lockfile. The app displays the package manifest version, so no UI version string needs changing.

Write actual changes and limitations in `docs/releases/<version>.md`. Merge the checked PR to main before tagging:

```sh
git switch main
git pull --ff-only
git tag -a v0.2.0 -m "Tether 0.2.0"
git push origin v0.2.0
```

**Publish release** requires a matching version tag reachable from main. All four targets run the shared checks, build signed packages, and test their packaged MCP executables. The final job alone gets `contents: write`, verifies every checksum/signature file and platform, and generates `latest.json` with version, plain-text notes, date, URLs, and signatures for all four native updater targets.

The job uploads sixteen assets to a temporary draft, verifies the uploaded inventory, then publishes it. A `-prerelease` tag creates a prerelease without replacing the stable latest channel. A failed build leaves no public partial release. An interrupted draft can be resumed; already published releases are left unchanged. The built-in GitHub token is used, and the workflow never invents a tag.

Retry an existing tag through Actions or:

```sh
gh workflow run release.yml --ref main -f tag=v0.2.0
```

Installed apps check the latest stable release's `latest.json` through **Settings → Check for updates**. Installation is user initiated. A native guard checks again after download and blocks new sessions during installation. The updater verifies the downloaded package against the bundled key; remote release notes are never evaluated as HTML.

## Validation and screenshots

```sh
npm run check:version
npm run format:check
npm run build
npm test
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run test:ui
# After a native build:
npm run test:native-mcp
# Windows WebView2: real IPC, approval, command execution, xterm PTY, revocation, update guard
npm run test:native-ui
npm run docs:screenshots
```

Native MCP verification runs on every packaged platform. Windows desktop behavior is additionally exercised through WebView2 locally; browser checks and MCP checks do not certify every native macOS/Linux desktop interaction. The optional live tunnel test requires internet access and a verified sidecar, so it is excluded from deterministic CI.

Documentation images capture the real browser-preview interface with a generic folder and no sessions, credentials, or client data. Use Edge on Windows or Playwright Chromium elsewhere. Commit refreshed images with UI changes.
