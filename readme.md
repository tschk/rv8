# RV8

A standalone browser-engine repo for Soliloquy and Roverite, combining Servo rendering with V8 JavaScript execution.

## Related repos

| Repo | Role |
|------|------|
| **this repo** (`atechnology-company/rv8`) | Canonical browser engine: multi-process IPC, Servo embed, V8, storage, extensions, `viewportd` |
| [`atechnology-company/soliloquy`](https://github.com/atechnology-company/soliloquy) | Appliance runtime; in-tree `src/rv8` stays aligned with this repo |
| `roverite` | Desktop app shell (`src/bin/roverite.rs`) |

`atechnology-company/rover` is archived (single-process engine + `rover-proto` contracts); its useful ideas are covered by the RV8 engine + `ipc/messages.rs`.

## Architecture

RV8 uses a Chrome-like multi-process architecture:

```
┌─────────────────────────────────────────────────────────┐
│                   Browser Process                       │
│  • Tab Management    • Navigation    • Process Control  │
│  • Extension Runtime   • WebExtensions API adapter      │
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
- **WebExtensions API Adapter**: Manifest V2/V3 + WebKit/Chromium/Firefox namespace parity scaffold
- **Chrome-like Optimizations**: Tab discarding, prefetching, code caching
- **Multi-Process**: Sandboxed renderers, site isolation
- **Modern Standards**: HTTP/3, Web APIs, DevTools Protocol

## `viewportd` (host embed protocol)

Roverite and other GPUI shells spawn `viewportd` as a subprocess to avoid linking Servo into the UI crate.

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

Product-side clients should stay thin (`roverite`); engine behavior, extensions, and polyfills belong here in `rv8`.

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
├── extensions/         # WebExtensions API adapter
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

Product adapters and sync protocols live outside this engine repo; the RV8 engine exposes the extension and browser APIs consumed by Roverite and Soliloquy.

## Integrates with the Plates ecosystem (optional)

## 4 different views
- Arc
- Chrome/Standard
- Minimal
- and our own take on the browser