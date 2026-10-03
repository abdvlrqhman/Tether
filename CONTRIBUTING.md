# Development conventions

Copyright © 2026 Spacie. All rights reserved. This document describes internal development; it does not grant an open-source license.

Follow the setup and verification steps in the README. Keep Rust formatted with rustfmt and frontend/CLI/configuration files formatted with Prettier. Check TypeScript, Clippy, and native platform tests before review.

Domain changes must stay independent of Tauri, networking, filesystem paths, and process APIs. Application services own policy orchestration and depend on traits in `application/ports.rs`. Infrastructure implements those contracts. UI, HTTP, CLI, and MCP handlers adapt input/output and call use cases.

Add meaningful tests around authorization transitions, cancellation, transport boundaries, and platform behavior when changing those areas. Avoid tests that merely repeat implementation details. Host approval must remain a local operation; adding an agent protocol must not introduce a permission bypass.

Do not commit invitations, session files, tokens, private client data, downloaded binaries, or generated build artifacts. Review dependency and sidecar updates, keep their lockfiles/digests reproducible, and preserve required third-party notices. Native release signing belongs to Spacie's release infrastructure.

Describe changes with the concrete problem and resulting behavior, then relevant validation and limitations. Run a native smoke test on each affected OS before claiming support.
