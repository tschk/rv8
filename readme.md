# RV8

A standalone browser-engine repo for Soliloquy, combining Servo rendering with V8 JavaScript execution.

## Related repos

| Repo | Role |
|------|------|
| [`atechnology-company/rover`](https://github.com/atechnology-company/rover) | Canonical engine workspace extracted from Soliloquy (`rover-core`, `rover-proto`, Servo + V8 integration) |
| [`atechnology-company/rover-desktop`](https://github.com/atechnology-company/rover-desktop) | Desktop host (Xilem/Masonry UI) around `rover` |
| [`atechnology-company/soliloquy`](https://github.com/atechnology-company/soliloquy) | Appliance runtime, Alpine packaging, in-tree `src/rv8` |
| **this repo** (`atechnology-company/rv8`) | Multi-process engine scaffold; keep aligned with `src/rv8` until `rover` subsumes active development |

Clone the active engine stack:

```sh
gh repo clone atechnology-company/rover
gh repo clone atechnology-company/rover-desktop
```

`rover-desktop` expects `rover` as a sibling directory. A different project (macOS launcher) may also use the folder name `rover`; do not confuse it with the engine checkout.

## Architecture

RV8 uses a Chrome-like multi-process architecture:

```
┌─────────────────────────────────────────────────────────┐
│                   Browser Process                       │
│  • Tab Management    • Navigation    • Process Control  │
├─────────────────────────────────────────────────────────┤
│                    IPC Channels                         │
├─────────────────────────────────────────────────────────┤
│  ┌──────────────────┐  ┌──────────────────────────────┐│
│  │ Renderer Process │  │           GPU Process        ││
│  │ (per tab)        │  │         (Compositor)         ││
│  │ • HTML/CSS Parse │  │  • Layer Compositing         ││
│  │ • Layout         │  │  • Hardware Acceleration     ││
│  │ • V8 JavaScript  │  └──────────────────────────────┘│
│  └──────────────────┘                                   │
│  ┌──────────────────────────────────────────────────┐  │
│  │              Network Process                      │  │
│  │         • HTTP/HTTPS • Caching • Cookies          │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## Features

- **V8 JavaScript Engine**: TurboFan + Sparkplug compilation
- **Servo Rendering**: WebRender-based GPU rendering
- **Chrome-like Optimizations**: Tab discarding, prefetching, code caching
- **Multi-Process**: Sandboxed renderers, site isolation
- **Modern Standards**: HTTP/3, Web APIs, DevTools Protocol

## Quick Start

```bash
# Build
cargo build

# Run
cargo run -- https://example.com

# Run with single-process (debugging)
cargo run --features single-process -- https://example.com
```

## Structure

```
.
├── lib.rs              # Library entry
├── main.rs             # Binary entry (multi-process)
├── core/               # Browser process
│   ├── browser.rs      # Main browser coordinator
│   ├── tab.rs          # Tab management
│   ├── config.rs       # Configuration
│   └── process_manager.rs # Child process spawning
├── renderer/           # Renderer process (Servo-based)
├── js/                 # JavaScript engine (V8)
├── compositor/         # GPU compositing
├── networking/         # Network stack
├── storage/            # sled persistence (cookies, session, profile)
├── ipc/                # Inter-process communication
└── optimizations/      # Performance tuning
    ├── flags.rs        # Chrome-like optimization flags
    ├── monitor.rs      # Performance monitoring
    └── preload.rs      # Resource prefetching
```

## Storage

Persistent browser state lives under the profile directory in `storage.sled` (sled-backed). Incognito mode uses in-memory stores only.

| Subsystem | Module | Notes |
|-----------|--------|-------|
| Profile metadata | `storage/profile.rs` | Profile id and meta tree |
| Cookies | `storage/cookie.rs` | `CookieJar` with insert/get/replace |
| Session | `storage/session.rs` | Tab snapshots per profile |

Profile paths are defined in `core/config.rs` (`BrowserDataDirs`); see [AGENTS.md](./AGENTS.md) for env vars and defaults.

```rust
use rv8::StorageManager;
use std::path::Path;

let storage = StorageManager::open(Path::new("/var/lib/soliloquy/browser/profiles/default"), false)?;
storage.cookies.insert(cookie)?;
storage.flush().await;
```

Cross-device sync and product adapters belong in [mono](https://github.com/atechnology-company/mono) (`mono-adapters`, feature `rv8`), not in this engine repo.

## Integrates with the Plates ecosystem (optional)

## 4 different views
- Arc
- Chrome/Standard
- Minimal
- and our own take on the browser