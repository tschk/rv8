//! Minimal DOM/JS shims injected into every document so modern SPAs boot under Servo.

pub const SCRIPT: &str = r#"
(function () {
  var root = this;
  if (root.__rv8EmbedPolyfills) return;
  root.__rv8EmbedPolyfills = true;

  if (typeof root.ResizeObserver === "undefined") {
    var registry = [];
    var tick = function () {
      for (var i = 0; i < registry.length; i++) {
        try {
          registry[i]._notify();
        } catch (_) {}
      }
      if (typeof root.requestAnimationFrame === "function") {
        root.requestAnimationFrame(tick);
      }
    };
    root.ResizeObserver = function ResizeObserver(callback) {
      this._callback = callback;
      this._targets = [];
      registry.push(this);
    };
    root.ResizeObserver.prototype.observe = function (target) {
      if (target && this._targets.indexOf(target) === -1) {
        this._targets.push(target);
      }
    };
    root.ResizeObserver.prototype.unobserve = function (target) {
      var index = this._targets.indexOf(target);
      if (index !== -1) {
        this._targets.splice(index, 1);
      }
    };
    root.ResizeObserver.prototype.disconnect = function () {
      this._targets = [];
      var index = registry.indexOf(this);
      if (index !== -1) {
        registry.splice(index, 1);
      }
    };
    root.ResizeObserver.prototype._notify = function () {
      if (!this._targets.length) return;
      var entries = [];
      for (var i = 0; i < this._targets.length; i++) {
        var target = this._targets[i];
        var rect;
        try {
          rect = target.getBoundingClientRect();
        } catch (_) {
          continue;
        }
        entries.push({
          target: target,
          contentRect: rect,
          borderBoxSize: [],
          contentBoxSize: [],
          devicePixelContentBoxSize: [],
        });
      }
      if (entries.length) this._callback(entries, this);
    };
    if (typeof root.requestAnimationFrame === "function") {
      root.requestAnimationFrame(tick);
    }
  }

  if (typeof root.queueMicrotask === "undefined") {
    root.queueMicrotask = function (callback) {
      Promise.resolve().then(callback).catch(function () {});
    };
  }
})();
"#;
