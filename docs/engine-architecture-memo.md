# RV8 engine architecture memo

## Decision

Keep RV8 as a Servo-based browser engine with a narrow RV8 embedder API. Do not restart as greenfield, do not import product sync, and do not grow gateway/mesh concerns in this repo.

## Comparison

| Engine | Useful lesson | RV8 action |
| --- | --- | --- |
| Servo upstream | Rust engine pieces, WebRender, Stylo, embeddable WebView path. | Keep submodule pinned, test in-process boot, upstream-compatible patches where possible. |
| Ladybird | Clean subsystem boundaries and independent engine implementation. | Copy boundary discipline, not C++ stack or greenfield scope. |
| luna-softworks/browser | No reliable public repo found by search. | Ignore until exact URL exists. Do not base architecture on unknown code. |
| Chromium | Browser process owns policy; renderer processes stay sandboxable and disposable. | Keep RV8 browser/process manager as policy owner. Add tests around renderer lifecycle before expanding IPC. |
| WebKit | Stable embedder layer around WebCore/JSC/WebKit2. | Expose RV8's small `BrowserEngine` facade and keep Servo details behind it. |
| Gecko | Separate style/layout/rendering and modern GPU display list pipeline. | Preserve Servo's style/layout/render split; avoid coupling RV8 storage/network/product code into layout. |

## Recommendations

1. Keep `servo-render` as the production rendering path and keep the software DOM path only as lightweight fallback/test scaffolding.
2. Treat `BrowserEngine` as the host API boundary: navigate, evaluate script, capture frame, resize, state getters. Add only methods real embedders need.
3. Keep Servo changes isolated in `support/js_stub` unless a failing test proves a higher layer needs changes.
4. Maintain three smoke gates:
   - JS realm smoke: `data_page_renders_with_layout` checks `document.readyState`
   - Render smoke: `data_page_renders_with_layout`
   - Embedder API smoke: `engine_uses_configured_viewport`
5. Keep process boundaries boring: browser owns tabs, storage, policy, and renderer lifecycle; renderer owns page execution and pixels.
6. Do not add `mono-protocol`, sync envelopes, mesh, gateway, device trust, or 2FA flows here.

## Next work

- Add a process-manager smoke that opens a tab through `Browser` and observes one rendered frame.
- Add a CI job for `cargo test -p rv8 --features servo-render --lib` on a host with software GL.
- Add one cross-platform boot benchmark once the render smoke is stable on Linux and macOS.
