# Development and release workflows

The same reusable check pipeline validates pull requests, manual installer builds, and tagged releases. No signing identities or release tokens are needed for ordinary checks and artifact builds.

## Daily development

1. Create a branch from `main` and make the change.
2. Run `npm run check:version`, `npm run format:check`, `npm run check`, Rust formatting/Clippy, and the relevant UI/native tests.
3. Open a pull request. **Checks** runs on Windows x64, macOS arm64/x64, and Linux x64.
4. Require the **Required checks** status in GitHub's branch rules if enforcing protected merges. This aggregate fails when any platform fails or is cancelled.
5. Merge after review. A push to `main` reruns the same checks.

Workflow actions are pinned by commit SHA, repository checkout credentials are not persisted, jobs have timeouts, and verification has read-only repository access. Pull requests do not receive release secrets. Rust and npm caches shorten later runs. Failed UI runs upload diagnostics for seven days; installer artifacts expire after fourteen days.

Dependency updates are opened weekly by Dependabot for npm, Cargo, and GitHub Actions. Review them through the same checks; updates are not merged automatically. Cloudflared is separately pinned in `scripts/cloudflared-lock.json` and must be updated with reviewed official asset checksums.

## Build installers without releasing

Open **Actions → Build installers → Run workflow** and select the branch to build, or run:

```sh
gh workflow run installers.yml --ref main
```

Each platform runs all checks before packaging. The built executable then passes the native stdio MCP smoke test. Successful jobs attach `tether-windows-x64`, `tether-macos-arm64`, `tether-macos-x64`, and `tether-linux-x64` artifacts, containing the installer and a platform-specific SHA-256 checksum file.

The macOS DMG contains the app. The Linux artifact contains `.deb` and `.AppImage`. Windows provides an NSIS `.exe`. These are unsigned development builds. The workflow does not publish a release or deploy anything.

## Prepare a versioned draft

Keep these versions aligned:

- `package.json`
- `package-lock.json` (both the top-level version and root package entry)
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock` (Tether's package entry; regenerate through Cargo)
- `src-tauri/tauri.conf.json`

`npm run check:version` checks the application manifest versions. Cargo's `--locked` checks enforce the Rust lockfile. Commit the version update and validated changes before tagging. For example, when all manifests are `0.2.0`:

```sh
git tag -a v0.2.0 -m "Tether 0.2.0"
git push origin v0.2.0
```

The tag triggers **Prepare draft release**. A mismatch between tag and manifest version stops the pipeline. The shared checks build all installers and test their MCP executables; only after success does a separate job receive `contents: write` to create a **draft** GitHub release with installers and checksums.

The workflow uses GitHub's built-in `GITHUB_TOKEN`, not a personal token. It verifies that the tag exists and never creates a tag implicitly. A rerun fails if a release for that tag already exists; inspect the existing draft instead of overwriting published assets.

## Customer distribution

The supplied release workflow prepares unsigned drafts. Before distributing trusted customer builds, integrate Spacie's Windows signing and Apple's Developer ID signing/notarization into the packaging job, with secrets limited to trusted release runs. Do not expose signing identities to pull requests. Tauri's official [Windows signing](https://v2.tauri.app/distribute/sign/windows/) and [macOS signing](https://v2.tauri.app/distribute/sign/macos/) guides describe the platform setup.

Review the draft notes, verify checksums, install and exercise the desktop app on every target OS, and validate host/operator approval, PTY interaction, TryCloudflare routing, cancellation, expiry, and shutdown. Native MCP testing alone does not certify the desktop runtime. Replace the release-note template with the actual changes and limitations, then publish the reviewed draft through GitHub.

## Documentation images

```sh
npm run docs:screenshots
```

This captures the real React interface in a browser preview with a generic folder and no live connection. It uses Edge on Windows and Playwright Chromium elsewhere. Install Chromium with `npx playwright install chromium` when needed. Commit the updated `docs/images` files alongside UI changes; never capture live invitations, credentials, or client data for public documentation.
