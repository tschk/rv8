# RV8 contributor guide

RV8 is a multi-process browser **engine** (Servo rendering + V8 JS). Like an AOSP base platform crate: storage, IPC, renderer, networking — **no** cross-device sync, **no** `mono-protocol`, **no** gateway or mesh.

Product sync and adapters live only in [`atechnology-company/mono`](https://github.com/atechnology-company/mono) (`crates/mono-adapters`, feature `rv8`).

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

`BrowserConfig::user_data_dir` mirrors `profile_dir`. Persistent storage opens `{profile_dir}/storage.sled` unless `incognito` is true (in-memory only).

## Architecture (high level)

```
main.rs          → process dispatch (browser / renderer / gpu / network)
core/            → Browser, tabs, config, process manager
storage/         → sled-backed cookies, session, profile meta
networking/      → NetworkManager (reqwest wiring TODO)
servo_embed/     → DOM/parser/embedder stubs
renderer/        → renderer subprocess
compositor/      → GPU process stubs
js/              → V8 engine wrapper
ipc/             → ipc-channel messages
```

## Boundaries (do not add here)

- No `mono-protocol`, `mono-mesh`, `mono-gateway`, or sync envelopes.
- No `TransferClass`, device trust, or 2FA flows.
- Mono consumes RV8 via path dependency + `mono-adapters::rv8` only.

## Code style

- Match existing module layout and `Result<_, String>` at browser boundaries; use `StorageError` inside `storage/`.
- Prefer minimal, focused diffs; no drive-by refactors.
- Run `cargo test` and `cargo build` before opening a PR.

## Related repos

- **mono** — sync protocol, adapters, gateway, browser product shell
- **rover** — launcher/runtime consumer
- **soliloquy** — appliance runtime; consumes this RV8 engine checkout
