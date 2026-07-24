//! Chrome DevTools Protocol (CDP) session for V8 debugging
//!
//! Handles JSON-RPC requests for Runtime, Debugger, Console, and Page domains.

use crate::js::JsEngine;
use crate::js::JsValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// CDP JSON-RPC error payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CdpError {
    pub code: i64,
    pub message: String,
}

/// CDP JSON-RPC request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CdpRequest {
    pub id: Option<u64>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// CDP JSON-RPC response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CdpResponse {
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CdpError>,
}

#[derive(Debug, Clone)]
struct Breakpoint {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    url: String,
    #[allow(dead_code)]
    line_number: u64,
}

/// CDP session state (does not own the engine).
#[derive(Default)]
pub struct CdpSessionState {
    runtime_enabled: bool,
    debugger_enabled: bool,
    console_enabled: bool,
    page_enabled: bool,
    paused: bool,
    breakpoints: HashMap<String, Breakpoint>,
    next_breakpoint_id: u64,
}

impl CdpSessionState {
    pub fn new() -> Self {
        Self {
            next_breakpoint_id: 1,
            ..Default::default()
        }
    }

    /// Parse a CDP JSON-RPC request string and return a JSON response string.
    pub fn cdp_send(&mut self, engine: &mut JsEngine, json: &str) -> String {
        match serde_json::from_str::<CdpRequest>(json) {
            Ok(request) => {
                let response = self.handle_request(engine, request);
                serde_json::to_string(&response).unwrap_or_else(|err| {
                    self.error_response(None, -32700, format!("Failed to serialize response: {err}"))
                })
            }
            Err(err) => self.error_response(None, -32700, format!("Parse error: {err}")),
        }
    }

    fn handle_request(&mut self, engine: &mut JsEngine, request: CdpRequest) -> CdpResponse {
        let id = request.id;
        match request.method.as_str() {
            "Runtime.enable" => {
                self.runtime_enabled = true;
                self.ok(id, serde_json::json!({}))
            }
            "Runtime.disable" => {
                self.runtime_enabled = false;
                self.ok(id, serde_json::json!({}))
            }
            "Runtime.evaluate" => self.runtime_evaluate(engine, id, &request.params),
            "Runtime.getProperties" => self.runtime_get_properties(id, &request.params),

            "Debugger.enable" => {
                self.debugger_enabled = true;
                self.ok(
                    id,
                    serde_json::json!({
                        "debuggerId": "rv8-debugger"
                    }),
                )
            }
            "Debugger.disable" => {
                self.debugger_enabled = false;
                self.breakpoints.clear();
                self.paused = false;
                self.ok(id, serde_json::json!({}))
            }
            "Debugger.setBreakpointByUrl" => self.debugger_set_breakpoint_by_url(id, &request.params),
            "Debugger.removeBreakpoint" => self.debugger_remove_breakpoint(id, &request.params),
            "Debugger.pause" => {
                self.paused = true;
                self.ok(id, serde_json::json!({}))
            }
            "Debugger.resume" => {
                self.paused = false;
                self.ok(id, serde_json::json!({}))
            }
            "Debugger.stepOver" => self.ok(id, serde_json::json!({})),
            "Debugger.stepInto" => self.ok(id, serde_json::json!({})),
            "Debugger.stepOut" => self.ok(id, serde_json::json!({})),

            "Console.enable" => {
                self.console_enabled = true;
                self.ok(id, serde_json::json!({}))
            }
            "Console.disable" => {
                self.console_enabled = false;
                self.ok(id, serde_json::json!({}))
            }

            "Page.enable" => {
                self.page_enabled = true;
                self.ok(id, serde_json::json!({}))
            }
            "Page.getFrameTree" => self.page_get_frame_tree(id),

            _ => CdpResponse {
                id,
                result: None,
                error: Some(CdpError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                }),
            },
        }
    }

    fn runtime_evaluate(
        &mut self,
        engine: &mut JsEngine,
        id: Option<u64>,
        params: &serde_json::Value,
    ) -> CdpResponse {
        let expression = match params.get("expression").and_then(|value| value.as_str()) {
            Some(expression) => expression,
            None => {
                return CdpResponse {
                    id,
                    result: None,
                    error: Some(CdpError {
                        code: -32602,
                        message: "Missing required parameter: expression".to_string(),
                    }),
                };
            }
        };

        match engine.execute(expression) {
            Ok(value) => {
                let remote_object = js_value_to_remote_object(&value);
                self.ok(
                    id,
                    serde_json::json!({
                        "result": remote_object,
                        "exceptionDetails": null
                    }),
                )
            }
            Err(message) => self.ok(
                id,
                serde_json::json!({
                    "result": {
                        "type": "undefined"
                    },
                    "exceptionDetails": {
                        "text": message
                    }
                }),
            ),
        }
    }

    fn runtime_get_properties(&mut self, id: Option<u64>, params: &serde_json::Value) -> CdpResponse {
        let object_id = params
            .get("objectId")
            .and_then(|value| value.as_str())
            .unwrap_or("");

        let properties = if object_id.is_empty() {
            Vec::new()
        } else {
            vec![serde_json::json!({
                "name": "value",
                "value": {
                    "type": "string",
                    "value": object_id
                },
                "configurable": true,
                "enumerable": true,
                "writable": true
            })]
        };

        self.ok(id, serde_json::json!({ "result": properties }))
    }

    fn debugger_set_breakpoint_by_url(
        &mut self,
        id: Option<u64>,
        params: &serde_json::Value,
    ) -> CdpResponse {
        let line_number = params
            .get("lineNumber")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let url = params
            .get("url")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();

        let breakpoint_id = format!("bp:{}", self.next_breakpoint_id);
        self.next_breakpoint_id += 1;

        self.breakpoints.insert(
            breakpoint_id.clone(),
            Breakpoint {
                id: breakpoint_id.clone(),
                url: url.clone(),
                line_number,
            },
        );

        self.ok(
            id,
            serde_json::json!({
                "breakpointId": breakpoint_id,
                "locations": [{
                    "scriptId": "1",
                    "lineNumber": line_number,
                    "columnNumber": 0
                }]
            }),
        )
    }

    fn debugger_remove_breakpoint(&mut self, id: Option<u64>, params: &serde_json::Value) -> CdpResponse {
        if let Some(breakpoint_id) = params.get("breakpointId").and_then(|value| value.as_str()) {
            self.breakpoints.remove(breakpoint_id);
        }
        self.ok(id, serde_json::json!({}))
    }

    fn page_get_frame_tree(&self, id: Option<u64>) -> CdpResponse {
        self.ok(
            id,
            serde_json::json!({
                "frameTree": {
                    "frame": {
                        "id": "main",
                        "loaderId": "main-loader",
                        "url": "about:blank",
                        "domainAndRegistry": "",
                        "securityOrigin": "://",
                        "mimeType": "text/html",
                        "secureContextType": "Secure",
                        "crossOriginIsolatedContextType": "NotIsolated",
                        "gatedAPIFeatures": []
                    },
                    "childFrames": []
                }
            }),
        )
    }

    fn ok(&self, id: Option<u64>, result: serde_json::Value) -> CdpResponse {
        CdpResponse {
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error_response(&self, id: Option<u64>, code: i64, message: String) -> String {
        let response = CdpResponse {
            id,
            result: None,
            error: Some(CdpError { code, message }),
        };
        serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"error":{"code":-32700,"message":"Internal JSON error"}}"#.to_string()
        })
    }
}

/// CDP session backed by a live V8 engine (convenience wrapper for tests).
pub struct CdpSession {
    engine: JsEngine,
    state: CdpSessionState,
}

impl CdpSession {
    pub fn new(engine: JsEngine) -> Self {
        Self {
            engine,
            state: CdpSessionState::new(),
        }
    }

    pub fn cdp_send(&mut self, json: &str) -> String {
        self.state.cdp_send(&mut self.engine, json)
    }
}

fn js_value_to_remote_object(value: &JsValue) -> serde_json::Value {
    match value {
        JsValue::Undefined => serde_json::json!({ "type": "undefined" }),
        JsValue::Null => serde_json::json!({ "type": "object", "subtype": "null", "value": null }),
        JsValue::Boolean(boolean) => serde_json::json!({ "type": "boolean", "value": boolean }),
        JsValue::Number(number) => serde_json::json!({ "type": "number", "value": number }),
        JsValue::String(string) => serde_json::json!({ "type": "string", "value": string }),
        JsValue::Object => serde_json::json!({ "type": "object", "className": "Object" }),
        JsValue::Array => serde_json::json!({ "type": "object", "subtype": "array", "className": "Array" }),
        JsValue::Function => {
            serde_json::json!({ "type": "function", "className": "Function" })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::js::JsEngine;

    #[test]
    fn test_cdp_request_response_types() {
        let request = CdpRequest {
            id: Some(1),
            method: "Runtime.enable".to_string(),
            params: serde_json::json!({}),
        };
        let json = serde_json::to_string(&request).unwrap();
        let parsed: CdpRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, request);
    }

    #[test]
    fn test_runtime_enable_disable() {
        let engine = JsEngine::new().unwrap();
        let mut session = CdpSession::new(engine);

        let enable = session.cdp_send(r#"{"id":1,"method":"Runtime.enable","params":{}}"#);
        let enable_response: CdpResponse = serde_json::from_str(&enable).unwrap();
        assert_eq!(enable_response.id, Some(1));
        assert!(enable_response.error.is_none());
        assert!(enable_response.result.is_some());

        let disable = session.cdp_send(r#"{"id":2,"method":"Runtime.disable","params":{}}"#);
        let disable_response: CdpResponse = serde_json::from_str(&disable).unwrap();
        assert_eq!(disable_response.id, Some(2));
        assert!(disable_response.error.is_none());
    }

    #[test]
    fn test_runtime_evaluate() {
        let mut engine = JsEngine::new().unwrap();
        engine.execute("var answer = 42;").unwrap();
        let mut session = CdpSession::new(engine);

        let response_json = session.cdp_send(
            r#"{"id":3,"method":"Runtime.evaluate","params":{"expression":"answer + 1","returnByValue":true}}"#,
        );
        let response: CdpResponse = serde_json::from_str(&response_json).unwrap();
        assert_eq!(response.id, Some(3));
        assert!(response.error.is_none());

        let result = response.result.expect("expected result");
        let remote_object = result.get("result").expect("expected nested result");
        assert_eq!(remote_object.get("type").and_then(|v| v.as_str()), Some("number"));
        assert_eq!(remote_object.get("value").and_then(|v| v.as_f64()), Some(43.0));
    }

    #[test]
    fn test_runtime_get_properties() {
        let engine = JsEngine::new().unwrap();
        let mut session = CdpSession::new(engine);

        let response_json = session.cdp_send(
            r#"{"id":4,"method":"Runtime.getProperties","params":{"objectId":"obj-1","ownProperties":true}}"#,
        );
        let response: CdpResponse = serde_json::from_str(&response_json).unwrap();
        assert_eq!(response.id, Some(4));
        let properties = response
            .result
            .and_then(|value| value.get("result").cloned())
            .and_then(|value| value.as_array().cloned())
            .expect("expected properties array");
        assert_eq!(properties.len(), 1);
        assert_eq!(
            properties[0].get("name").and_then(|value| value.as_str()),
            Some("value")
        );
    }

    #[test]
    fn test_debugger_methods() {
        let engine = JsEngine::new().unwrap();
        let mut session = CdpSession::new(engine);

        let enable = session.cdp_send(r#"{"id":5,"method":"Debugger.enable","params":{}}"#);
        let enable_response: CdpResponse = serde_json::from_str(&enable).unwrap();
        assert_eq!(
            enable_response
                .result
                .and_then(|value| {
                    value
                        .get("debuggerId")
                        .and_then(|id| id.as_str())
                        .map(str::to_string)
                })
                .as_deref(),
            Some("rv8-debugger")
        );

        let set_bp = session.cdp_send(
            r#"{"id":6,"method":"Debugger.setBreakpointByUrl","params":{"lineNumber":10,"url":"file:///test.js"}}"#,
        );
        let set_bp_response: CdpResponse = serde_json::from_str(&set_bp).unwrap();
        let breakpoint_id = set_bp_response
            .result
            .and_then(|value| {
                value
                    .get("breakpointId")
                    .and_then(|id| id.as_str())
                    .map(str::to_string)
            })
            .unwrap();

        let remove_bp = session.cdp_send(&format!(
            r#"{{"id":7,"method":"Debugger.removeBreakpoint","params":{{"breakpointId":"{breakpoint_id}"}}}}"#
        ));
        let remove_bp_response: CdpResponse = serde_json::from_str(&remove_bp).unwrap();
        assert!(remove_bp_response.error.is_none());

        for (request_id, method) in [
            (8, "Debugger.pause"),
            (9, "Debugger.resume"),
            (10, "Debugger.stepOver"),
            (11, "Debugger.stepInto"),
            (12, "Debugger.stepOut"),
        ] {
            let response_json =
                session.cdp_send(&format!(r#"{{"id":{request_id},"method":"{method}","params":{{}}}}"#));
            let response: CdpResponse = serde_json::from_str(&response_json).unwrap();
            assert_eq!(response.id, Some(request_id));
            assert!(response.error.is_none());
        }

        let disable = session.cdp_send(r#"{"id":13,"method":"Debugger.disable","params":{}}"#);
        let disable_response: CdpResponse = serde_json::from_str(&disable).unwrap();
        assert!(disable_response.error.is_none());
    }

    #[test]
    fn test_console_enable_disable() {
        let engine = JsEngine::new().unwrap();
        let mut session = CdpSession::new(engine);

        let enable = session.cdp_send(r#"{"id":14,"method":"Console.enable","params":{}}"#);
        let enable_response: CdpResponse = serde_json::from_str(&enable).unwrap();
        assert!(enable_response.error.is_none());

        let disable = session.cdp_send(r#"{"id":15,"method":"Console.disable","params":{}}"#);
        let disable_response: CdpResponse = serde_json::from_str(&disable).unwrap();
        assert!(disable_response.error.is_none());
    }

    #[test]
    fn test_page_methods() {
        let engine = JsEngine::new().unwrap();
        let mut session = CdpSession::new(engine);

        let enable = session.cdp_send(r#"{"id":16,"method":"Page.enable","params":{}}"#);
        let enable_response: CdpResponse = serde_json::from_str(&enable).unwrap();
        assert!(enable_response.error.is_none());

        let frame_tree = session.cdp_send(r#"{"id":17,"method":"Page.getFrameTree","params":{}}"#);
        let frame_tree_response: CdpResponse = serde_json::from_str(&frame_tree).unwrap();
        let frame = frame_tree_response
            .result
            .and_then(|value| value.get("frameTree").cloned())
            .and_then(|value| value.get("frame").cloned())
            .expect("expected frame");
        assert_eq!(frame.get("id").and_then(|value| value.as_str()), Some("main"));
        assert_eq!(
            frame.get("url").and_then(|value| value.as_str()),
            Some("about:blank")
        );
    }

    #[test]
    fn test_unknown_method() {
        let engine = JsEngine::new().unwrap();
        let mut session = CdpSession::new(engine);

        let response_json =
            session.cdp_send(r#"{"id":18,"method":"Unknown.method","params":{}}"#);
        let response: CdpResponse = serde_json::from_str(&response_json).unwrap();
        assert_eq!(response.id, Some(18));
        assert!(response.result.is_none());
        assert_eq!(
            response.error.as_ref().map(|error| error.code),
            Some(-32601)
        );
    }

    #[test]
    fn test_invalid_json() {
        let engine = JsEngine::new().unwrap();
        let mut session = CdpSession::new(engine);

        let response_json = session.cdp_send("{not json");
        let response: CdpResponse = serde_json::from_str(&response_json).unwrap();
        assert!(response.result.is_none());
        assert_eq!(
            response.error.as_ref().map(|error| error.code),
            Some(-32700)
        );
    }
}
