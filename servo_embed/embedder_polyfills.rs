//! Minimal DOM/JS shims injected into every document so modern SPAs boot under Servo.

pub const SCRIPT: &str = r#"
(function () {
  if (globalThis.__rv8EmbedPolyfills) return;
  globalThis.__rv8EmbedPolyfills = true;

  if (typeof globalThis.ResizeObserver === "undefined") {
    const registry = new Set();
    const tick = () => {
      for (const ro of registry) {
        try {
          ro._notify();
        } catch (_) {}
      }
      if (typeof globalThis.requestAnimationFrame === "function") {
        globalThis.requestAnimationFrame(tick);
      }
    };
    globalThis.ResizeObserver = class ResizeObserver {
      constructor(callback) {
        this._callback = callback;
        this._targets = new Set();
        registry.add(this);
      }
      observe(target) {
        if (target) this._targets.add(target);
      }
      unobserve(target) {
        this._targets.delete(target);
      }
      disconnect() {
        this._targets.clear();
        registry.delete(this);
      }
      _notify() {
        if (!this._targets.size) return;
        const entries = [];
        for (const target of this._targets) {
          let rect;
          try {
            rect = target.getBoundingClientRect();
          } catch (_) {
            continue;
          }
          entries.push({
            target,
            contentRect: rect,
            borderBoxSize: [],
            contentBoxSize: [],
            devicePixelContentBoxSize: [],
          });
        }
        if (entries.length) this._callback(entries, this);
      }
    };
    if (typeof globalThis.requestAnimationFrame === "function") {
      globalThis.requestAnimationFrame(tick);
    }
  }

  if (typeof globalThis.queueMicrotask === "undefined") {
    globalThis.queueMicrotask = function (callback) {
      Promise.resolve().then(callback).catch(function () {});
    };
  }
})();
"#;
