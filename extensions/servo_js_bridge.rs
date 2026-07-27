//! WebExtensions background script JS bridge for servo-render (direct rusty_v8).
//!
//! Spawns one V8 engine thread per extension and exposes a native function
//! `__rv8_extension_api` that the injected `chrome` / `browser` shim calls.
//! This bridge lives in the main browser process, separate from the Servo
//! renderer process, so it uses its own V8 isolate and avoids colliding with
//! Servo's js_stub/soliloquy_v8 isolate in the renderer.

use log::{info, warn};
use rusty_v8 as v8;
use serde_json::{json, Value};
use std::sync::mpsc::{channel, Sender};
use std::sync::Weak;
use std::thread::{self, JoinHandle};

use crate::extensions::api::ApiRequest;
use crate::extensions::runtime::{ExtensionId, ExtensionRuntime};

const SHIM: &str = r#"
(function() {
    function rv8_api(namespace, method, args) {
        var jsonArgs = JSON.stringify(args);
        var extra = Array.prototype.slice.call(arguments, 3);
        var result = __rv8_extension_api(namespace, method, jsonArgs, extra[0]);
        if (result === undefined) return undefined;
        try {
            return JSON.parse(result);
        } catch (e) {
            throw new Error(result);
        }
    }
    function makeRuntime() {
        return {
            id: function() { return rv8_api('runtime', 'id', []); },
            getManifest: function() { return rv8_api('runtime', 'getManifest', []); },
            getURL: function(path) { return rv8_api('runtime', 'getURL', [path]); },
            getPlatformInfo: function() { return rv8_api('runtime', 'getPlatformInfo', []); },
            sendMessage: function(message, options, responseCallback) {
                return rv8_api('runtime', 'sendMessage', [message]);
            },
            onMessage: {
                addListener: function(listener) {
                    rv8_api('runtime', 'onMessage.addListener', [], listener);
                },
                removeListener: function(listener) {
                    rv8_api('runtime', 'onMessage.removeListener', [], listener);
                },
                hasListener: function() { return false; },
                hasListeners: function() { return false; }
            }
        };
    }
    function makeScripting() {
        return {
            executeScript: function(injection) {
                var arg;
                if (typeof injection === 'string') {
                    arg = injection;
                } else if (injection && typeof injection.func === 'function') {
                    arg = { code: '(' + injection.func.toString() + ')();' };
                } else if (injection && typeof injection.code === 'string') {
                    arg = { code: injection.code };
                } else {
                    arg = injection;
                }
                return rv8_api('scripting', 'executeScript', [arg]);
            }
        };
    }
    var chrome = { runtime: makeRuntime(), scripting: makeScripting() };
    var browser = { runtime: makeRuntime(), scripting: makeScripting() };
    if (typeof globalThis !== 'undefined') {
        globalThis.chrome = chrome;
        globalThis.browser = browser;
    } else {
        var self = this;
        self.chrome = chrome;
        self.browser = browser;
    }
})();
"#;

pub struct BackgroundScriptRuntime {
    sender: Sender<BridgeCommand>,
    thread: Option<JoinHandle<()>>,
}

enum BridgeCommand {
    Shutdown,
}

impl BackgroundScriptRuntime {
    pub fn new(
        extension_id: ExtensionId,
        scripts: Vec<String>,
        runtime: Weak<ExtensionRuntime>,
    ) -> Self {
        let (tx, rx) = channel();
        let handle = thread::spawn(move || {
            let mut isolate = match create_isolate() {
                Ok(i) => i,
                Err(e) => {
                    warn!(
                        "Failed to create V8 isolate for {}: {}",
                        extension_id.0, e
                    );
                    return;
                }
            };
            let context = {
                let handle_scope = &mut v8::HandleScope::new(&mut isolate);
                let context = v8::Context::new(handle_scope);
                v8::Global::new(handle_scope, context)
            };
            let mut state = BridgeState {
                extension_id,
                runtime,
                listeners: Vec::new(),
            };
            if let Err(e) = install_bridge(&mut isolate, &context, &mut state) {
                warn!(
                    "Failed to install bridge for {}: {}",
                    state.extension_id.0, e
                );
                return;
            }
            for script in scripts {
                if let Err(e) = execute_script_string(&mut isolate, &context, &script) {
                    warn!(
                        "Background script error for {}: {}",
                        state.extension_id.0, e
                    );
                } else {
                    info!("Executed background script for {}", state.extension_id.0);
                }
            }
            #[allow(clippy::never_loop)]
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    BridgeCommand::Shutdown => break,
                }
            }
        });
        Self {
            sender: tx,
            thread: Some(handle),
        }
    }

    pub fn shutdown(&mut self) {
        let _ = self.sender.send(BridgeCommand::Shutdown);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for BackgroundScriptRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct BridgeState {
    extension_id: ExtensionId,
    runtime: Weak<ExtensionRuntime>,
    listeners: Vec<v8::Global<v8::Function>>,
}

fn create_isolate() -> Result<v8::OwnedIsolate, String> {
    js::ensure_v8();
    info!("V8 background script engine initialized for servo-render");
    Ok(v8::Isolate::new(v8::CreateParams::default()))
}

fn execute_script_string(
    isolate: &mut v8::OwnedIsolate,
    context: &v8::Global<v8::Context>,
    script: &str,
) -> Result<String, String> {
    let handle_scope = &mut v8::HandleScope::new(isolate);
    let context_local = v8::Local::new(handle_scope, context);
    let scope = &mut v8::ContextScope::new(handle_scope, context_local);

    let code = v8::String::new(scope, script).ok_or("Failed to create script string")?;
    let script_obj = v8::Script::compile(scope, code, None).ok_or("Failed to compile script")?;
    let result = script_obj
        .run(scope)
        .ok_or("Script execution returned no value")?;
    result
        .to_string(scope)
        .map(|s| s.to_rust_string_lossy(scope))
        .ok_or_else(|| "Failed to stringify result".to_string())
}

fn v8_value_to_string(scope: &mut v8::HandleScope, value: v8::Local<v8::Value>) -> Option<String> {
    value
        .to_string(scope)
        .map(|s| s.to_rust_string_lossy(scope))
}

fn v8_value_to_json_string(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
) -> Option<String> {
    if value.is_undefined() {
        return None;
    }
    if let Some(s) = v8::json::stringify(scope, value) {
        Some(s.to_rust_string_lossy(scope))
    } else {
        value
            .to_string(scope)
            .map(|s| s.to_rust_string_lossy(scope))
    }
}

fn send_response_noop(
    _scope: &mut v8::HandleScope,
    _args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
}

fn send_message(
    scope: &mut v8::HandleScope,
    state: &mut BridgeState,
    message: Value,
    rv: &mut v8::ReturnValue,
) {
    let message_json = serde_json::to_string(&message).unwrap_or_else(|_| "null".into());
    let message_v8_str = v8::String::new(scope, &message_json)
        .unwrap_or_else(|| v8::String::new(scope, "null").expect("null string"));
    let message_v8 = v8::json::parse(scope, message_v8_str)
        .map(|v| v as v8::Local<v8::Value>)
        .unwrap_or_else(|| v8::undefined(scope).into());

    let sender = json!({
        "id": state.extension_id.0,
        "url": "",
        "tab": null,
        "frameId": 0
    });
    let sender_json = serde_json::to_string(&sender).unwrap_or_else(|_| "{}".into());
    let sender_v8_str = v8::String::new(scope, &sender_json)
        .unwrap_or_else(|| v8::String::new(scope, "{}").expect("empty object string"));
    let sender_v8 = v8::json::parse(scope, sender_v8_str)
        .map(|v| v as v8::Local<v8::Value>)
        .unwrap_or_else(|| v8::undefined(scope).into());

    let send_response: v8::Local<v8::Value> =
        if let Some(f) = v8::Function::new(scope, send_response_noop) {
            f.into()
        } else {
            v8::undefined(scope).into()
        };

    let context = scope.get_current_context();
    let recv: v8::Local<v8::Value> = context.global(scope).into();

    let listeners: Vec<v8::Global<v8::Function>> = state.listeners.clone();
    for listener in listeners {
        let local = v8::Local::new(scope, listener);
        let result = local.call(scope, recv, &[message_v8, sender_v8, send_response]);
        if let Some(result) = result {
            if result.boolean_value(scope) {
                if let Some(json) = v8_value_to_json_string(scope, result) {
                    if let Some(s) = v8::String::new(scope, &json) {
                        rv.set(s.into());
                    }
                    return;
                }
            }
        }
    }
}

fn handle_api(
    scope: &mut v8::HandleScope,
    state: &mut BridgeState,
    namespace: &str,
    method: &str,
    args_json: &str,
    rv: &mut v8::ReturnValue,
) {
    let runtime = match state.runtime.upgrade() {
        Some(r) => r,
        None => return,
    };
    let args = match serde_json::from_str::<Value>(args_json) {
        Ok(Value::Array(arr)) => arr,
        Ok(Value::Null) => Vec::new(),
        Ok(other) => vec![other],
        Err(_) => Vec::new(),
    };
    let req = ApiRequest::new(namespace, method, state.extension_id.clone()).with_args(args);
    let out = match runtime.call_api(req) {
        Ok(value) => serde_json::to_string(&value).unwrap_or_default(),
        Err(e) => e,
    };
    if let Some(s) = v8::String::new(scope, &out) {
        rv.set(s.into());
    }
}

fn handle_scripting(
    scope: &mut v8::HandleScope,
    state: &mut BridgeState,
    method: &str,
    args_json: &str,
    rv: &mut v8::ReturnValue,
) {
    if method != "executeScript" {
        handle_api(scope, state, "scripting", method, args_json, rv);
        return;
    }
    let parsed = serde_json::from_str::<Value>(args_json).unwrap_or(Value::Null);
    let code = parsed
        .as_array()
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or(Value::Null);
    let code_str = match code {
        Value::String(s) => s,
        Value::Object(mut o) => {
            if let Some(Value::String(s)) = o.remove("code") {
                s
            } else if let Some(Value::String(s)) = o.remove("func") {
                format!("({})();", s)
            } else {
                String::new()
            }
        }
        _ => String::new(),
    };
    let result_json = if code_str.is_empty() {
        "null".to_string()
    } else {
        let code_v8 = v8::String::new(scope, &code_str).unwrap();
        let result = v8::Script::compile(scope, code_v8, None).and_then(|s| s.run(scope));
        result
            .and_then(|v| v8_value_to_json_string(scope, v))
            .unwrap_or_else(|| "null".into())
    };
    let out = format!("[{{\"result\":{}}}]", result_json);
    if let Some(s) = v8::String::new(scope, &out) {
        rv.set(s.into());
    }
}

fn handle_runtime(
    scope: &mut v8::HandleScope,
    args: &v8::FunctionCallbackArguments,
    state: &mut BridgeState,
    method: &str,
    args_json: &str,
    rv: &mut v8::ReturnValue,
) {
    if method == "onMessage.addListener" {
        if let Ok(listener) = v8::Local::<v8::Function>::try_from(args.get(3)) {
            state.listeners.push(v8::Global::new(scope, listener));
        }
        return;
    }
    if method == "onMessage.removeListener" {
        if let Ok(listener) = v8::Local::<v8::Function>::try_from(args.get(3)) {
            let listener_val: v8::Local<v8::Value> = listener.into();
            state.listeners.retain(|g| {
                let local = v8::Local::new(scope, g.clone());
                let local_val: v8::Local<v8::Value> = local.into();
                !listener_val.strict_equals(local_val)
            });
        }
        return;
    }
    if method == "sendMessage" {
        let parsed = serde_json::from_str::<Value>(args_json).unwrap_or(Value::Null);
        let message = parsed
            .as_array()
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null);
        send_message(scope, state, message, rv);
        return;
    }
    handle_api(scope, state, "runtime", method, args_json, rv);
}

fn extension_api_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Some(data) = args.data() else {
        return;
    };
    let Ok(external) = v8::Local::<v8::External>::try_from(data) else {
        return;
    };
    let state = unsafe { &mut *(external.value() as *mut BridgeState) };

    let Some(namespace) = v8_value_to_string(scope, args.get(0)) else {
        return;
    };
    let Some(method) = v8_value_to_string(scope, args.get(1)) else {
        return;
    };
    let args_json = v8_value_to_string(scope, args.get(2)).unwrap_or_default();

    match namespace.as_str() {
        "runtime" => handle_runtime(scope, &args, state, &method, &args_json, &mut rv),
        "scripting" => handle_scripting(scope, state, &method, &args_json, &mut rv),
        _ => handle_api(scope, state, &namespace, &method, &args_json, &mut rv),
    }
}

fn install_bridge(
    isolate: &mut v8::OwnedIsolate,
    context: &v8::Global<v8::Context>,
    state: &mut BridgeState,
) -> Result<(), String> {
    let handle_scope = &mut v8::HandleScope::new(isolate);
    let context_local = v8::Local::new(handle_scope, context);
    let scope = &mut v8::ContextScope::new(handle_scope, context_local);
    let global = context_local.global(scope);

    let external = v8::External::new(scope, state as *mut BridgeState as *mut std::ffi::c_void);
    let api_fn = v8::Function::builder(extension_api_callback)
        .data(external.into())
        .build(scope)
        .ok_or("Failed to create __rv8_extension_api")?;
    let name = v8::String::new(scope, "__rv8_extension_api")
        .ok_or("Failed to create API function name")?;
    let _ = global.set(scope, name.into(), api_fn.into());

    let shim = v8::String::new(scope, SHIM).ok_or("Failed to create shim string")?;
    let script = v8::Script::compile(scope, shim, None).ok_or("Failed to compile shim")?;
    let _ = script.run(scope);
    Ok(())
}
