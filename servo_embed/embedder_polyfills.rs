//! Minimal DOM/JS shims injected into every document so modern SPAs boot under Servo.
//!
//! Injected after page navigation before any application script runs.
//! These are JS-level polyfills that fill gaps in Servo's Web API surface.
//! Rust-level V8 bindings (console, timers, storage) live in js/ for the
//! standalone `rv8-v8` feature path. This file is for the servo-render path.

pub const SCRIPT: &str = r#"
(function () {
  var root = this;
  if (root.__rv8EmbedPolyfills) return;
  root.__rv8EmbedPolyfills = true;

  // ── ResizeObserver ──
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

  // ── queueMicrotask ──
  if (typeof root.queueMicrotask === "undefined") {
    root.queueMicrotask = function (callback) {
      Promise.resolve().then(callback).catch(function () {});
    };
  }

  // ── console — route to Rust tracing::info! via __rv8ConsoleLog ──
  if (typeof root.console === "undefined") {
    root.console = {};
  }
  var c = root.console;
  function fmtArgs(args) {
    var parts = [];
    for (var i = 0; i < args.length; i++) {
      var v = args[i];
      if (typeof v === "undefined") parts.push("undefined");
      else if (v === null) parts.push("null");
      else if (typeof v === "string") parts.push(v);
      else if (typeof v === "object") {
        try { parts.push(JSON.stringify(v)); } catch (_) { parts.push(String(v)); }
      } else parts.push(String(v));
    }
    return parts.join(" ");
  }
  function makeLog(level) {
    return function () { var msg = fmtArgs(arguments); if (msg && typeof __rv8ConsoleLog === "function") __rv8ConsoleLog(level, msg); };
  }
  c.log = makeLog("info");
  c.info = makeLog("info");
  c.warn = makeLog("warn");
  c.error = makeLog("error");
  c.debug = makeLog("debug");
  c.trace = makeLog("trace");
  c.dir = c.log;
  c.group = function () {};
  c.groupEnd = function () {};
  c.time = function () {};
  c.timeEnd = function () {};
  c.assert = function (cond) { if (!cond) c.error("assertion failed"); };

  // ── setTimeout / setInterval — fire callbacks via microtask ──
  if (typeof root.setTimeout === "undefined") {
    var timerId = 1;
    var timers = {};
    root.setTimeout = function (fn, ms) {
      var id = timerId++;
      timers[id] = { fn: fn, interval: false, delay: Math.max(ms || 0, 0) };
      scheduleTimerTick();
      return id;
    };
    root.clearTimeout = function (id) {
      delete timers[id];
    };
    root.setInterval = function (fn, ms) {
      var id = timerId++;
      timers[id] = { fn: fn, interval: true, delay: Math.max(ms || 1, 1) };
      scheduleTimerTick();
      return id;
    };
    root.clearInterval = root.clearTimeout;
    var tickScheduled = false;
    function scheduleTimerTick() {
      if (tickScheduled) return;
      tickScheduled = true;
      Promise.resolve().then(function () {
        tickScheduled = false;
        var now = Date.now();
        var ids = Object.keys(timers);
        for (var i = 0; i < ids.length; i++) {
          var t = timers[ids[i]];
          if (!t) continue;
          try {
            if (typeof t.fn === "function") t.fn();
          } catch (_) {}
          if (!t.interval) delete timers[ids[i]];
        }
      });
    }
  }

  // ── fetch — polyfill via XMLHttpRequest ──
  if (typeof root.fetch === "undefined" && typeof root.XMLHttpRequest !== "undefined") {
    root.fetch = function (url, opts) {
      opts = opts || {};
      var method = (opts.method || "GET").toUpperCase();
      var headers = opts.headers || {};
      var body = opts.body || null;
      return new Promise(function (resolve, reject) {
        var xhr = new XMLHttpRequest();
        xhr.open(method, url, true);
        xhr.withCredentials = true;
        // Set headers
        if (typeof headers === "object") {
          for (var k in headers) {
            if (headers.hasOwnProperty(k)) {
              xhr.setRequestHeader(k, headers[k]);
            }
          }
        }
        xhr.onload = function () {
          var respHeaders = {};
          var hdr = xhr.getAllResponseHeaders() || "";
          hdr.split("\r\n").forEach(function (line) {
            var idx = line.indexOf(":");
            if (idx > 0) {
              respHeaders[line.substring(0, idx).toLowerCase()] = line.substring(idx + 2);
            }
          });
          resolve(new Response(xhr.responseText, {
            status: xhr.status,
            statusText: xhr.statusText,
            headers: respHeaders,
          }));
        };
        xhr.onerror = function () { reject(new TypeError("Network fetch failed")); };
        xhr.ontimeout = function () { reject(new TypeError("Fetch timeout")); };
        xhr.send(body);
      });
    };
    // Minimal Response polyfill needed for fetch
    if (typeof root.Response === "undefined") {
      root.Response = function (body, init) {
        init = init || {};
        this.body = body;
        this.status = init.status || 200;
        this.statusText = init.statusText || "OK";
        this.ok = this.status >= 200 && this.status < 300;
        this.headers = init.headers || {};
        this._bodyText = body;
      };
      root.Response.prototype.text = function () { return Promise.resolve(String(this._bodyText)); };
      root.Response.prototype.json = function () { return Promise.resolve(JSON.parse(this._bodyText)); };
      root.Response.prototype.blob = function () { return Promise.resolve(new Blob([this._bodyText])); };
    }
  }

  // ── localStorage / sessionStorage ──
  function makeStorage() {
    var store = {};
    return {
      get length() { return Object.keys(store).length; },
      key: function (i) { return Object.keys(store)[i] || null; },
      getItem: function (k) { return store.hasOwnProperty(k) ? store[k] : null; },
      setItem: function (k, v) { store[k] = String(v); },
      removeItem: function (k) { delete store[k]; },
      clear: function () { store = {}; }
    };
  }
  if (typeof root.localStorage === "undefined") {
    root.localStorage = makeStorage();
  }
  if (typeof root.sessionStorage === "undefined") {
    root.sessionStorage = makeStorage();
  }
})();
"#;
