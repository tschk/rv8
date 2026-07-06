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

  // ── console — polyfill missing methods ──
  if (typeof root.console === "undefined") {
    root.console = {};
  }
  var c = root.console;
  var noop = function () {};
  var methods = ["log","warn","error","info","debug","trace","dir","group","groupEnd","time","timeEnd","assert"];
  for (var i = 0; i < methods.length; i++) {
    if (typeof c[methods[i]] === "undefined") c[methods[i]] = noop;
  }

  // ── setTimeout / clearTimeout ──
  if (typeof root.setTimeout === "undefined") {
    var timerId = 1;
    var timers = {};
    root.setTimeout = function (fn, ms) {
      var id = timerId++;
      timers[id] = { fn: fn, interval: false };
      return id;
    };
    root.clearTimeout = function (id) {
      delete timers[id];
    };
    root.setInterval = function (fn, ms) {
      var id = timerId++;
      timers[id] = { fn: fn, interval: true };
      return id;
    };
    root.clearInterval = function (id) {
      delete timers[id];
    };
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
