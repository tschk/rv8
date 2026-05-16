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
├── rv8_sync/           # mono-protocol-shaped export/import
├── ipc/                # Inter-process communication
└── optimizations/      # Performance tuning
    ├── flags.rs        # Chrome-like optimization flags
    ├── monitor.rs      # Performance monitoring
    └── preload.rs      # Resource prefetching
```

## Mono browser platform integration

RV8 is being prepared as a **product adapter** for [mono](https://github.com/atechnology-company/mono) (`mono-protocol` replicated objects), alongside Plates/Rover.

| Area | Location | Status |
|------|----------|--------|
| Profile + sled DB | `storage/` → `{profile_dir}/storage.sled` | **Implemented** (cookies, session, profile meta) |
| Mono-shaped export/import | `rv8_sync/` | **Implemented** (JSON envelopes; no crypto/2FA yet) |
| `BrowserEngine` in mono-browser | external crate | **TODO** — wire `Rv8EngineSync` to `mono-protocol` types |
| Network cookie injection | `networking/` | **TODO** — reqwest jar backed by `CookieJar` |

Profile paths are defined in `core/config.rs` (`BrowserDataDirs`); see [AGENTS.md](./AGENTS.md) for env vars and defaults.

Example sync export (local dev):

```rust
use rv8::{export_cookie_jar_json, import_cookie_jar, StorageManager};

let storage = StorageManager::open(profile_dir, false)?;
let json = export_cookie_jar_json(&storage, "identity-id")?;
import_cookie_jar(&storage, "identity-id", &json)?;
```

## Integrates with the Plates ecosystem (optional)

## 4 different views
- Arc
- Chrome/Standard
- Minimal
- and our own take on the browser