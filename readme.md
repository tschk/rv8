# RV8

A standalone browser-engine repo for Soliloquy, combining Servo rendering with V8 JavaScript execution.

## Related repos

| Repo | Role |
|------|------|
| **this repo** (`atechnology-company/rv8`) | Canonical browser engine: multi-process IPC, Servo embed, V8, storage, `viewportd` |
| [`atechnology-company/soliloquy`](https://github.com/atechnology-company/soliloquy) | Appliance runtime; in-tree `src/rv8` stays aligned with this repo |

Archived experiments (read-only, not maintained): `atechnology-company/rover` (single-process engine + `rover-proto` contracts) and `atechnology-company/rover-desktop` (thin Xilem shell). Their useful ideas—stable host service traits and `DocumentSnapshot`—are already covered here by `ipc/messages.rs` plus richer process/storage/Servo paths. Do not confuse those repos with the unrelated macOS launcher at `semitechnological/rover`.

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

## `viewportd` (host embed protocol)

GPUI shells (e.g. [mono-browser](https://github.com/atechnology-company/mono)) spawn `viewportd` as a subprocess to avoid linking Servo into the UI crate.

```bash
cargo build -p rv8 --bin viewportd
RV8_VIEWPORT_WIDTH=1280 RV8_VIEWPORT_HEIGHT=800 ./target/debug/viewportd
```

**Stdin (line-based):**

| Command | Example |
|---------|---------|
| Navigate | `NAV https://example.com/` |
| Resize | `SIZE 1280 800` |
| Scroll | `SCROLL 0.0 120.0` (device pixels; positive `y` reveals content below) |
| Quit | `QUIT` |

**Stdout:** `RV8M` metadata frames (title/url), then `RV8F` length-prefixed RGBA frames. See `src/bin/viewportd.rs` and `servo_embed/viewport.rs`.

Product-side clients should stay thin (`mono-adapters`); engine behavior and polyfills belong here in `servo_embed/`.

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