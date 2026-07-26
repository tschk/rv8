//! JavaScript value types

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JsValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Object,
    Array,
    Function,
}

impl JsValue {
    pub fn to_json(&self) -> Value {
        match self {
            JsValue::Undefined | JsValue::Null => Value::Null,
            JsValue::Boolean(b) => Value::Bool(*b),
            JsValue::Number(n) => serde_json::Number::from_f64(*n)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            JsValue::String(s) => Value::String(s.clone()),
            JsValue::Object | JsValue::Array | JsValue::Function => {
                Value::String("[object]".to_string())
            }
        }
    }
}
