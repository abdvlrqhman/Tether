# Security model

Tether is an attended remote debugging tool with unrestricted user-level shell access after local approval. Use it only on devices whose owner authorized the debugging session.

## Trust boundaries

- No listening server or tunnel runs until the local user starts sharing.
- The HTTP listener is loopback-only; the optional bundled tunnel supplies the Internet route.
- An invitation is a cryptographically random 256-bit secret, valid for ten minutes. Possession allows a pairing request only.
- The local app approves a pending request for a fixed duration. Approval consumes the invitation and issues a separate 256-bit session token. Other pending requests are denied.
- Pairing proofs and session tokens are checked through SHA-256 hashes with constant-time comparison. Delivery tokens are temporarily retained in memory to let the approved operator finish polling; revocation erases them.
- Expiry, host revocation, operator logout, and stopping the host invalidate authorization. Managed processes are terminated using platform process scopes. No elevated privileges are requested.
- The desktop's session credentials stay in Rust memory. CLI credentials are private local files with Unix permissions or Windows ACLs. Neither client forwards bearer tokens across HTTP redirects.
- There are no remotely callable approval endpoints or generic frontend shell/filesystem plugins. Tauri capability access is local-window-only. The production CSP excludes external scripts, frames, and direct remote networking.
- Native API calls reject browser Origin headers; this reduces browser-origin attacks but does not establish client identity. Authentication remains mandatory.
- Request/body/frame/output/job/process bounds limit accidental or unauthenticated resource consumption. Pairing has a global attempt budget; a leaked public hostname can still receive denial-of-service traffic.

## What authorization means

The starting folder is convenience, not a security boundary. An approved operator can read secrets, change files, install tools, and start services that the user account can access. Verify the operator through a trusted channel; names are self-reported. Do not run Tether with administrator/root privileges for ordinary debugging.

Revocation prevents further API use and terminates supervised shells and ordinary descendants. It cannot undo file edits, externally launched services, scheduled tasks, or other persistent changes already made by an authorized operator. Unix processes can deliberately escape a process group; the tool is not a malware containment system. Windows Job Object assignment follows process creation; shell access itself remains the primary trust grant.

Cloudflare terminates HTTPS/WSS. Traffic is encrypted in transit, but the provider can inspect it; this version does not implement end-to-end encryption against the relay. Loopback HTTP is for local testing, not unencrypted LAN use. Quick Tunnels provide temporary hostnames without production availability guarantees.

Audit logs record approvals and process lifecycle, not terminal/command content. They are local append-only JSONL, not cryptographically tamper-evident logs, and a user-level shell can alter them. The UI retains the latest 200 events; disk logs currently require operator retention/rotation. Required audit writes are checked before granting access or starting work; revocation always proceeds even if logging fails.

## Distribution

The sidecar download uses a pinned Cloudflare release and checked-in SHA-256 digests from official release metadata. Updating it requires reviewing the release, replacing version/digests, regenerating binaries, and rerunning native tests. Runtime automatic sidecar updates are disabled.

Tether update packages are signed with a dedicated updater key. The public key is bundled into the app; the private key stays outside the repository and is supplied only to installer/release packaging through GitHub Secrets. The native updater verifies downloaded signatures before installation. Checks are manual, release notes are rendered as plain text, and installing requires no shared or connected session. A native guard blocks new host/operator connections during installation.

Updater signatures do not replace Windows publisher signing or Apple code signing/notarization, which are not configured for these initial installers. Test the native desktop runtime on every platform you intend to support. Linux in-app installation requires an AppImage; Debian packages are updated through a new installer.

Report issues privately to Spacie through its established support channel at [spacie.net](https://spacie.net/). Include the Tether version, OS, reproduction steps, and sanitized logs. Never include invitation secrets, session files, or client data in a public report.
