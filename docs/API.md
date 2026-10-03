# Tether API v1

The host binds an ephemeral `127.0.0.1` port. The Internet route forwards that port through a temporary TryCloudflare HTTPS endpoint. HTTPS is required for non-loopback operator endpoints. Credentials go in JSON or `Authorization: Bearer …`, never query strings.

All responses have `Cache-Control: no-store`. Browser requests with an Origin header are rejected, and no CORS headers are enabled. Maximum request body is 32 KiB. Errors generally use `{"error":"message"}`; malformed JSON can be rejected by the HTTP framework before the handler runs.

## Pair

`POST /v1/pair`, JSON:

```json
{ "invite": "<invitation secret>", "operator": "Spacie agent" }
```

Returns `{"id":"<request id>","proof":"<polling credential>"}`. A valid invitation permits a request, not shell access. Operator names are self-reported, 1–80 bytes, without control characters. At most eight live requests and 30 attempts/minute are accepted. Invalid requests consume rate budget too.

`GET /v1/pair/{id}`, using the polling proof as bearer token, returns:

```json
{ "status": "pending", "token": null, "expires_at": null, "os": null }
```

After the local host approves it, `status` becomes `approved` and `token` contains the independently generated session bearer token. `expires_at` is Unix seconds. A denied or revoked request does not return a session token. Polling ends after the five-minute pairing-request deadline; use `/v1/session` after approval.

## Session

`GET /v1/session`, session bearer required, returns session id, operator label, deadline, OS, and starting directory. `DELETE /v1/session` revokes that session and terminates managed processes. No HTTP endpoint starts sharing, approves requests, renews invitations, or grants more time.

## Command jobs

`POST /v1/exec`, session bearer required:

```json
{ "command": "git status", "cwd": null, "timeout_seconds": 60 }
```

`command` is shell syntax, not an argv array. Windows uses PowerShell; Unix uses `/bin/sh`. A missing/null `cwd` uses the host's starting folder. This is not a folder restriction. Commands are limited to 16 KiB and timeouts to 1–300 seconds. A new process is used for each job, so changes to shell variables or current directory do not persist across jobs.

Returns `{"id":"<job id>","status":"running","result":null}`. Poll `GET /v1/jobs/{id}`. Cancel with `DELETE /v1/jobs/{id}`. The job must belong to the session. Cancellation is asynchronous; poll for its completed result if the session remains valid.

```json
{
  "id": "<job id>",
  "status": "finished",
  "result": {
    "stdout": "…",
    "stderr": "…",
    "exit_code": 0,
    "timed_out": false,
    "cancelled": false,
    "truncated": false,
    "error": null
  }
}
```

Combined output is limited to 1 MiB before UTF-8 decoding. Null exit code means no ordinary process exit code was available. Revoking a session immediately prevents further job reads, even while process cleanup is finishing. Four command jobs/terminals may run concurrently. Completed results are kept only in memory, with bounded retention; they are not audit records.

## Interactive terminal

Open `GET /v1/terminal` as WebSocket with a session bearer header. Browser WebSocket clients are intentionally unsupported; the desktop uses a native Rust socket and the CLI uses Node `ws`.

Client frames:

```json
{"type":"input","data":"<standard base64 of terminal bytes>"}
{"type":"resize","cols":120,"rows":30}
```

Server frames:

```json
{"type":"output","data":"<standard base64 of terminal bytes>"}
{"type":"error","message":"…"}
```

Input bytes are limited to 16 KiB per message; WebSocket messages and frames to 32 KiB. Terminal size is clamped to 2–500 rows/columns. Handle WebSocket ping/pong. PowerShell/terminal applications may request cursor position; a real terminal emulator must answer standard terminal queries. Closing the socket, revocation, expiry, or a stalled connection terminates its managed shell. Native operator events add a `closed` frame for the UI when the socket ends.

`GET /health` is public and only returns service/version information. It does not expose invitations, session state, device names, or directory paths.
