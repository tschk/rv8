# RV8 contributor guide

RV8 is a multi-process browser engine scaffold (Servo rendering + V8 JS) targeting Soliloquy appliance deployments and future **mono** sync via [`atechnology-company/mono`](https://github.com/atechnology-company/mono).

## Build and test

```bash
cargo build
cargo test
cargo run -- https://example.com
```

Single-process debugging:

```bash
cargo run --features single-process -- https://example.com
```

## Profile paths (`core/config.rs`)

Writable state is isolated from the read-only host image via `BrowserDataDirs`:

| Field | Env override | Default (appliance) |
|-------|----------------|---------------------|
| `profile_dir` | `SOLILOQUY_PROFILE_DIR` | `/var/lib/soliloquy/browser/profiles/default` |
| `cache_dir` | `SOLILOQUY_CACHE_DIR` | `/var/lib/soliloquy/browser/cache` |
| `downloads_dir` | `SOLILOQUY_DOWNLOADS_DIR` | `/var/lib/soliloquy/browser/downloads` |
| `state_dir` | `SOLILOQUY_STATE_DIR` | `/var/lib/soliloquy/browser/state` |
| `logs_dir` | `SOLILOQUY_LOG_DIR` | `/var/lib/soliloquy/browser/logs` |
| `terminal_state_dir` | `SOLILOQUY_TERMINAL_STATE_DIR` | `/var/lib/soliloquy/browser/terminal` |

`BrowserConfig::user_data_dir` mirrors `profile_dir`. Persistent RV8 storage opens `{profile_dir}/storage.sled` unless `incognito` is true (in-memory only).

## Architecture (high level)

```
main.rs          → process dispatch (browser / renderer / gpu / network)
core/            → Browser, tabs, config, process manager
storage/         → sled-backed cookies, session, profile meta
rv8_sync/        → mono-protocol-shaped export/import payloads
networking/      → NetworkManager (reqwest wiring TODO)
servo_embed/     → DOM/parser/embedder stubs
renderer/        → renderer subprocess
compositor/      → GPU process stubs
js/              → V8 engine wrapper
ipc/             → ipc-channel messages
```

## Mono integration points

| RV8 module | Mono concept | Status |
|------------|--------------|--------|
| `storage::CookieJar` | `CookieJarObject` (`SensitiveSession`) | Persisted in sled; export via `rv8_sync` |
| `storage::SessionStore` | `BrowserSession` (`PrivateState`) | Tab list + active tab snapshot |
| `rv8_sync::Rv8SyncAdapter` | `BrowserEngine::export/apply_object_payload` | JSON envelopes; no `mono-protocol` dep yet |
| `export_cookie_jar_json` / `import_cookie_jar` | `CookieJarObject` + inline jar snapshot | `MonoCookieJarEnvelope` JSON |
| `rv8_sync::Rv8EngineSync` | `TransferClass` string mapping | `sensitive_session` / `private_state` |

Read mono docs when extending sync:

- `mono/docs/OBJECTS.md` — `SyncObject`, transfer classes, residency
- `mono/docs/PROTOCOL.md` — envelopes, encryption, journals
- `mono/crates/mono-browser/src/engine.rs` — `BrowserEngine` trait

Next adapter steps (not in RV8 yet):

1. Add optional `mono-protocol` path dependency and map `Rv8SyncAdapter` to real `CookieJarObject` / encryption.
2. Wire `NetworkManager` reqwest client to `CookieJar` on requests.
3. Implement `BrowserEngine for Rv8Engine` inside `mono-browser` (product crate).

## Code style

- Match existing module layout and `Result<_, String>` at browser boundaries; use `StorageError` inside `storage/`.
- Prefer minimal, focused diffs; no drive-by refactors.
- Run `cargo test` and `cargo build` before opening a PR.

## Related repos

- **mono** — sync protocol, gateway, browser engine trait
- **rover** — canonical engine extraction target
- **soliloquy** — appliance runtime; in-tree `src/rv8` should stay aligned
