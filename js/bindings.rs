//! V8 bindings for DOM and Web APIs
//!
//! This module implements the mapping between V8 JavaScript objects
//! and the Rust implementation of DOM nodes and Web APIs.

use parking_lot::RwLock;
use rusty_v8 as v8;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::networking::{WebSocketConnection, WebSocketFrame, WebSocketManager, WebSocketState};
use crate::servo_embed::dom::{DomEvent, DomTree, NodeId, NodeType};
use crate::servo_embed::web_apis::{ConsoleApi, StorageApi, TimerManager};
use crate::storage::{DatabaseMetadata, IndexedDb, KeyRange, ObjectStoreMetadata};
use crate::js::worker::{V8Worker, WorkerMessage};

const CONTEXT_DATA_KEY: &str = "__rv8_context_data";
const NODE_ID_KEY: &str = "__rv8_node_id";
const STORAGE_TYPE_KEY: &str = "__rv8_storage_type";
const OBSERVER_ID_KEY: &str = "__rv8_observer_id";
const CANVAS_CTX_ID_KEY: &str = "__rv8_canvas_ctx_id";
const WEBSOCKET_ID_KEY: &str = "__rv8_websocket_id";
const WORKER_ID_KEY: &str = "__rv8_worker_id";
const IDB_DB_NAME_KEY: &str = "__rv8_idb_db_name";
const IDB_STORE_NAME_KEY: &str = "__rv8_idb_store_name";

static NEXT_OBSERVER_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_CANVAS_CTX_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_WORKER_ID: AtomicU64 = AtomicU64::new(1);

struct MutationObserverState {
    id: u64,
    callback: v8::Global<v8::Function>,
    targets: Vec<NodeId>,
    records: Vec<MutationRecord>,
}

#[derive(Clone)]
struct MutationRecord {
    target_id: NodeId,
    record_type: String,
}

struct IntersectionObserverState {
    id: u64,
    callback: v8::Global<v8::Function>,
    targets: Vec<NodeId>,
}

struct Canvas2DState {
    id: u64,
    node_id: NodeId,
    fill_style: String,
    stroke_style: String,
    path: Vec<CanvasPathOp>,
}

#[derive(Clone)]
enum CanvasPathOp {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    Arc(f64, f64, f64, f64, f64, bool),
}

/// Data stored in V8 context embedder data
pub struct V8ContextData {
    pub dom_tree: Arc<RwLock<DomTree>>,
    pub console_api: Arc<RwLock<ConsoleApi>>,
    pub timer_manager: Arc<RwLock<TimerManager>>,
    pub local_storage: Arc<RwLock<StorageApi>>,
    pub session_storage: Arc<RwLock<StorageApi>>,
    pub indexeddb: Arc<RwLock<IndexedDb>>,
    pub websocket_manager: Arc<RwLock<WebSocketManager>>,
    pub timer_callbacks: RwLock<HashMap<u64, v8::Global<v8::Function>>>,
    pub event_listeners: RwLock<HashMap<(NodeId, String), Vec<v8::Global<v8::Function>>>>,
    mutation_observers: RwLock<HashMap<u64, MutationObserverState>>,
    intersection_observers: RwLock<HashMap<u64, IntersectionObserverState>>,
    canvas_contexts: RwLock<HashMap<u64, Canvas2DState>>,
    workers: RwLock<HashMap<u64, V8Worker>>,
}

impl V8ContextData {
    pub fn new(
        dom_tree: Arc<RwLock<DomTree>>,
        console_api: Arc<RwLock<ConsoleApi>>,
        timer_manager: Arc<RwLock<TimerManager>>,
        local_storage: Arc<RwLock<StorageApi>>,
        session_storage: Arc<RwLock<StorageApi>>,
        indexeddb: Arc<RwLock<IndexedDb>>,
        websocket_manager: Arc<RwLock<WebSocketManager>>,
    ) -> Self {
        Self {
            dom_tree,
            console_api,
            timer_manager,
            local_storage,
            session_storage,
            indexeddb,
            websocket_manager,
            timer_callbacks: Default::default(),
            event_listeners: Default::default(),
            mutation_observers: Default::default(),
            intersection_observers: Default::default(),
            canvas_contexts: Default::default(),
            workers: Default::default(),
        }
    }
}

/// Initialize a V8 context with DOM and Web APIs
pub fn initialize_context<'s>(
    scope: &mut v8::HandleScope<'s, ()>,
    data: V8ContextData,
) -> v8::Local<'s, v8::Context> {
    let global_template = v8::ObjectTemplate::new(scope);

    let context = v8::Context::new_from_template(scope, global_template);
    let scope = &mut v8::ContextScope::new(scope, context);

    let data_ptr = Box::into_raw(Box::new(data));
    set_context_data(scope, data_ptr);

    // Set up DOM and Storage on the context
    setup_console(scope, context);
    setup_timers(scope, context);
    setup_dom(scope, context);
    setup_storage(scope, context);
    setup_indexeddb(scope, context);
    setup_websocket(scope, context);
    setup_workers(scope, context);

    context
}

/// Remove and free the Rust data attached to the current V8 context.
pub fn take_context_data(scope: &mut v8::HandleScope) -> Option<Box<V8ContextData>> {
    let ptr = context_data_ptr(scope)?;
    let global = scope.get_current_context().global(scope);
    let key = v8::String::new(scope, CONTEXT_DATA_KEY)?;
    let undefined = v8::undefined(scope);
    let _ = global.set(scope, key.into(), undefined.into());

    // SAFETY: `ptr` was created with `Box::into_raw` in `initialize_context`.
    Some(unsafe { Box::from_raw(ptr.as_ptr()) })
}

fn set_context_data<'s>(scope: &mut v8::HandleScope<'s>, data_ptr: *mut V8ContextData) {
    let global = scope.get_current_context().global(scope);
    let key = v8::String::new(scope, CONTEXT_DATA_KEY).expect("static V8 key should allocate");
    let external = v8::External::new(scope, data_ptr.cast::<c_void>());
    let _ = global.set(scope, key.into(), external.into());
}

fn context_data_ptr(scope: &mut v8::HandleScope) -> Option<std::ptr::NonNull<V8ContextData>> {
    let global = scope.get_current_context().global(scope);
    let key = v8::String::new(scope, CONTEXT_DATA_KEY)?;
    let value = global.get(scope, key.into())?;
    let external = v8::Local::<v8::External>::try_from(value).ok()?;
    std::ptr::NonNull::new(external.value().cast::<V8ContextData>())
}

fn set_property<'s>(
    scope: &mut v8::HandleScope<'s>,
    object: v8::Local<v8::Object>,
    name: &str,
    value: v8::Local<v8::Value>,
) {
    let key = v8::String::new(scope, name).expect("static V8 key should allocate");
    let _ = object.set(scope, key.into(), value);
}

fn set_number_property<'s>(
    scope: &mut v8::HandleScope<'s>,
    object: v8::Local<v8::Object>,
    name: &str,
    value: u64,
) {
    let number = v8::Number::new(scope, value as f64);
    set_property(scope, object, name, number.into());
}

fn get_number_property(
    scope: &mut v8::HandleScope,
    object: v8::Local<v8::Object>,
    name: &str,
) -> Option<u64> {
    let key = v8::String::new(scope, name)?;
    object
        .get(scope, key.into())?
        .integer_value(scope)
        .map(|value| value as u64)
}

fn value_to_string(scope: &mut v8::HandleScope, value: v8::Local<v8::Value>) -> Option<String> {
    value
        .to_string(scope)
        .map(|value| value.to_rust_string_lossy(scope))
}

fn setup_console<'s>(scope: &mut v8::HandleScope<'s>, context: v8::Local<v8::Context>) {
    let global = context.global(scope);
    let console = v8::Object::new(scope);

    let log_callback = v8::Function::new(scope, console_log).expect("console.log function");
    set_property(scope, console, "log", log_callback.into());

    let info_callback = v8::Function::new(scope, console_info).expect("console.info function");
    set_property(scope, console, "info", info_callback.into());

    let warn_callback = v8::Function::new(scope, console_warn).expect("console.warn function");
    set_property(scope, console, "warn", warn_callback.into());

    let error_callback = v8::Function::new(scope, console_error).expect("console.error function");
    set_property(scope, console, "error", error_callback.into());

    set_property(scope, global, "console", console.into());
}

fn setup_timers<'s>(scope: &mut v8::HandleScope<'s>, context: v8::Local<v8::Context>) {
    let global = context.global(scope);

    let set_timeout = v8::Function::new(scope, set_timeout_callback).expect("setTimeout function");
    set_property(scope, global, "setTimeout", set_timeout.into());

    let clear_timeout =
        v8::Function::new(scope, clear_timer_callback).expect("clearTimeout function");
    set_property(scope, global, "clearTimeout", clear_timeout.into());

    let set_interval =
        v8::Function::new(scope, set_interval_callback).expect("setInterval function");
    set_property(scope, global, "setInterval", set_interval.into());

    let clear_interval =
        v8::Function::new(scope, clear_timer_callback).expect("clearInterval function");
    set_property(scope, global, "clearInterval", clear_interval.into());
}

fn setup_storage<'s>(scope: &mut v8::HandleScope<'s>, context: v8::Local<v8::Context>) {
    let global = context.global(scope);
    let local_storage_obj = create_storage_object(scope, 0);
    let session_storage_obj = create_storage_object(scope, 1);

    set_property(scope, global, "localStorage", local_storage_obj.into());
    set_property(scope, global, "sessionStorage", session_storage_obj.into());
}

fn create_storage_object<'s>(
    scope: &mut v8::HandleScope<'s>,
    storage_type: u64,
) -> v8::Local<'s, v8::Object> {
    let object = v8::Object::new(scope);
    set_number_property(scope, object, STORAGE_TYPE_KEY, storage_type);

    let get_item_fn = v8::Function::new(scope, storage_get_item).expect("storage getItem function");
    set_property(scope, object, "getItem", get_item_fn.into());

    let set_item_fn = v8::Function::new(scope, storage_set_item).expect("storage setItem function");
    set_property(scope, object, "setItem", set_item_fn.into());

    let remove_item_fn =
        v8::Function::new(scope, storage_remove_item).expect("storage removeItem function");
    set_property(scope, object, "removeItem", remove_item_fn.into());

    let clear_fn = v8::Function::new(scope, storage_clear).expect("storage clear function");
    set_property(scope, object, "clear", clear_fn.into());

    object
}

fn setup_dom<'s>(scope: &mut v8::HandleScope<'s>, context: v8::Local<v8::Context>) {
    let global = context.global(scope);

    let doc_id = get_context_data(scope).dom_tree.read().document_id();
    let doc_obj = create_node_object(scope, doc_id);
    let create_element_fn =
        v8::Function::new(scope, create_element_callback).expect("document.createElement function");
    set_property(scope, doc_obj, "createElement", create_element_fn.into());

    let query_selector_fn =
        v8::Function::new(scope, query_selector_callback).expect("document.querySelector function");
    set_property(scope, doc_obj, "querySelector", query_selector_fn.into());

    set_property(scope, global, "document", doc_obj.into());

    let node_ctor = v8::Function::new(scope, empty_constructor).expect("Node constructor");
    let element_ctor = v8::Function::new(scope, empty_constructor).expect("Element constructor");
    let html_element_ctor =
        v8::Function::new(scope, empty_constructor).expect("HTMLElement constructor");
    let html_input_ctor =
        v8::Function::new(scope, empty_constructor).expect("HTMLInputElement constructor");
    let html_canvas_ctor =
        v8::Function::new(scope, empty_constructor).expect("HTMLCanvasElement constructor");
    let document_ctor = v8::Function::new(scope, empty_constructor).expect("Document constructor");

    set_property(scope, global, "Node", node_ctor.into());
    set_property(scope, global, "Element", element_ctor.into());
    set_property(scope, global, "HTMLElement", html_element_ctor.into());
    set_property(scope, global, "HTMLInputElement", html_input_ctor.into());
    set_property(scope, global, "HTMLCanvasElement", html_canvas_ctor.into());
    set_property(scope, global, "Document", document_ctor.into());

    let mutation_observer_ctor = v8::FunctionTemplate::new(scope, mutation_observer_constructor);
    let mutation_observer_ctor = mutation_observer_ctor.get_function(scope).expect("MutationObserver");
    set_property(scope, global, "MutationObserver", mutation_observer_ctor.into());

    let intersection_observer_ctor =
        v8::FunctionTemplate::new(scope, intersection_observer_constructor);
    let intersection_observer_ctor = intersection_observer_ctor
        .get_function(scope)
        .expect("IntersectionObserver");
    set_property(scope, global, "IntersectionObserver", intersection_observer_ctor.into());
}

fn setup_indexeddb<'s>(scope: &mut v8::HandleScope<'s>, context: v8::Local<v8::Context>) {
    let global = context.global(scope);
    let indexed_db = v8::Object::new(scope);
    let open_fn = v8::Function::new(scope, indexeddb_open_callback).expect("indexedDB.open");
    set_property(scope, indexed_db, "open", open_fn.into());
    set_property(scope, global, "indexedDB", indexed_db.into());
}

fn setup_websocket<'s>(scope: &mut v8::HandleScope<'s>, context: v8::Local<v8::Context>) {
    let global = context.global(scope);
    let ws_ctor = v8::FunctionTemplate::new(scope, websocket_constructor);
    let ws_ctor = ws_ctor.get_function(scope).expect("WebSocket constructor");
    set_property(scope, global, "WebSocket", ws_ctor.into());
}

fn setup_workers<'s>(scope: &mut v8::HandleScope<'s>, context: v8::Local<v8::Context>) {
    let global = context.global(scope);
    let worker_ctor = v8::FunctionTemplate::new(scope, worker_constructor);
    let worker_ctor = worker_ctor.get_function(scope).expect("Worker constructor");
    set_property(scope, global, "Worker", worker_ctor.into());
}

/// Wrap a NodeId into a JS object
pub fn wrap_node<'s>(
    scope: &mut v8::HandleScope<'s>,
    node_id: NodeId,
) -> v8::Local<'s, v8::Object> {
    create_node_object(scope, node_id)
}

fn empty_constructor(
    _scope: &mut v8::HandleScope,
    _args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
}

fn create_node_object<'s>(
    scope: &mut v8::HandleScope<'s>,
    node_id: NodeId,
) -> v8::Local<'s, v8::Object> {
    let object = v8::Object::new(scope);
    set_number_property(scope, object, NODE_ID_KEY, node_id);

    let tag_name = get_context_data(scope)
        .dom_tree
        .read()
        .get_node(node_id)
        .and_then(|n| n.tag_name.clone());

    if matches!(tag_name.as_deref(), Some("canvas")) {
        let get_context_fn =
            v8::Function::new(scope, canvas_get_context_callback).expect("getContext");
        set_property(scope, object, "getContext", get_context_fn.into());
    }

    if matches!(tag_name.as_deref(), Some("input")) {
        let value_getter = v8::String::new(scope, "value").expect("value property");
        let _ = object.set_accessor_with_setter(
            scope,
            value_getter.into(),
            input_value_getter,
            input_value_setter,
        );
    }

    let node_type_name = v8::String::new(scope, "nodeType").expect("nodeType property");
    let _ = object.set_accessor(scope, node_type_name.into(), node_type_getter);

    let node_name = v8::String::new(scope, "nodeName").expect("nodeName property");
    let _ = object.set_accessor(scope, node_name.into(), node_name_getter);

    let tag_name = v8::String::new(scope, "tagName").expect("tagName property");
    let _ = object.set_accessor(scope, tag_name.into(), tag_name_getter);

    let text_content = v8::String::new(scope, "textContent").expect("textContent property");
    let _ = object.set_accessor_with_setter(
        scope,
        text_content.into(),
        text_content_getter,
        text_content_setter,
    );

    let append_child =
        v8::Function::new(scope, append_child_callback).expect("appendChild function");
    set_property(scope, object, "appendChild", append_child.into());

    let remove_child =
        v8::Function::new(scope, remove_child_callback).expect("removeChild function");
    set_property(scope, object, "removeChild", remove_child.into());

    let set_attribute =
        v8::Function::new(scope, set_attribute_callback).expect("setAttribute function");
    set_property(scope, object, "setAttribute", set_attribute.into());

    let get_attribute =
        v8::Function::new(scope, get_attribute_callback).expect("getAttribute function");
    set_property(scope, object, "getAttribute", get_attribute.into());

    let add_event_listener =
        v8::Function::new(scope, add_event_listener_callback).expect("addEventListener function");
    set_property(scope, object, "addEventListener", add_event_listener.into());

    object
}

// --- Callbacks (Console, Timers, DOM, Storage) ---

fn console_log(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let message = value_to_string(scope, args.get(0)).unwrap_or_default();
    get_context_data(scope).console_api.write().log(&message);
}

fn console_info(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let message = value_to_string(scope, args.get(0)).unwrap_or_default();
    get_context_data(scope).console_api.write().info(&message);
}

fn console_warn(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let message = value_to_string(scope, args.get(0)).unwrap_or_default();
    get_context_data(scope).console_api.write().warn(&message);
}

fn console_error(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let message = value_to_string(scope, args.get(0)).unwrap_or_default();
    get_context_data(scope).console_api.write().error(&message);
}

fn set_timeout_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Ok(callback) = v8::Local::<v8::Function>::try_from(args.get(0)) else {
        return;
    };
    let delay = args.get(1).integer_value(scope).unwrap_or(0) as u64;

    let data = get_context_data(scope);
    let timer_id = data.timer_manager.write().set_timeout(0, delay);

    // Store the callback
    data.timer_callbacks
        .write()
        .insert(timer_id, v8::Global::new(scope, callback));

    rv.set(v8::Number::new(scope, timer_id as f64).into());
}

fn set_interval_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Ok(callback) = v8::Local::<v8::Function>::try_from(args.get(0)) else {
        return;
    };
    let interval = args.get(1).integer_value(scope).unwrap_or(0) as u64;

    let data = get_context_data(scope);
    let timer_id = data.timer_manager.write().set_interval(0, interval);

    // Store the callback
    data.timer_callbacks
        .write()
        .insert(timer_id, v8::Global::new(scope, callback));

    rv.set(v8::Number::new(scope, timer_id as f64).into());
}

fn clear_timer_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let timer_id = args.get(0).integer_value(scope).unwrap_or(0) as u64;
    let data = get_context_data(scope);
    data.timer_manager.write().clear_timer(timer_id);
    data.timer_callbacks.write().remove(&timer_id);
}

fn node_type_getter(
    scope: &mut v8::HandleScope,
    _name: v8::Local<v8::Name>,
    args: v8::PropertyCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Some(node_id) = get_number_property(scope, args.this(), NODE_ID_KEY) else {
        return;
    };
    let type_val = {
        let dom_tree = get_context_data(scope).dom_tree.read();
        dom_tree.get_node(node_id).map(|node| match node.node_type {
            NodeType::Element => 1,
            NodeType::Text => 3,
            NodeType::Comment => 8,
            NodeType::Document => 9,
            NodeType::DocumentFragment => 11,
        })
    };
    if let Some(type_val) = type_val {
        rv.set(v8::Integer::new(scope, type_val).into());
    }
}

fn node_name_getter(
    scope: &mut v8::HandleScope,
    _name: v8::Local<v8::Name>,
    args: v8::PropertyCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Some(node_id) = get_number_property(scope, args.this(), NODE_ID_KEY) else {
        return;
    };
    let dom_tree = get_context_data(scope).dom_tree.read();
    if let Some(node) = dom_tree.get_node(node_id) {
        let name = node.tag_name.as_deref().unwrap_or("#text");
        if let Some(v8_str) = v8::String::new(scope, name) {
            rv.set(v8_str.into());
        }
    }
}

fn tag_name_getter(
    scope: &mut v8::HandleScope,
    _name: v8::Local<v8::Name>,
    args: v8::PropertyCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Some(node_id) = get_number_property(scope, args.this(), NODE_ID_KEY) else {
        return;
    };
    let tag_name = {
        let dom_tree = get_context_data(scope).dom_tree.read();
        dom_tree
            .get_node(node_id)
            .and_then(|node| node.tag_name.as_ref().map(|tag| tag.to_uppercase()))
    };
    if let Some(tag_name) = tag_name {
        if let Some(v8_str) = v8::String::new(scope, &tag_name) {
            rv.set(v8_str.into());
        }
    }
}

fn text_content_getter(
    scope: &mut v8::HandleScope,
    _name: v8::Local<v8::Name>,
    args: v8::PropertyCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Some(node_id) = get_number_property(scope, args.this(), NODE_ID_KEY) else {
        return;
    };
    let v8_str = {
        let dom_tree = get_context_data(scope).dom_tree.read();
        let text = dom_tree
            .get_node(node_id)
            .and_then(|node| node.text_content.as_deref())
            .unwrap_or_default();
        v8::String::new(scope, text)
    };
    if let Some(value) = v8_str {
        rv.set(value.into());
    }
}

fn text_content_setter(
    scope: &mut v8::HandleScope,
    _name: v8::Local<v8::Name>,
    value: v8::Local<v8::Value>,
    args: v8::PropertyCallbackArguments,
) {
    let Some(node_id) = get_number_property(scope, args.this(), NODE_ID_KEY) else {
        return;
    };
    let Some(text) = value_to_string(scope, value) else {
        return;
    };
    get_context_data(scope)
        .dom_tree
        .write()
        .set_text_content(node_id, &text);
}

fn create_element_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let tag = value_to_string(scope, args.get(0)).unwrap_or_else(|| "div".to_string());
    let new_id = get_context_data(scope)
        .dom_tree
        .write()
        .create_element(&tag);
    rv.set(wrap_node(scope, new_id).into());
}

fn query_selector_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Some(selector) = value_to_string(scope, args.get(0)) else {
        rv.set(v8::null(scope).into());
        return;
    };

    let node_id = get_context_data(scope)
        .dom_tree
        .read()
        .query_selector(&selector);

    if let Some(node_id) = node_id {
        rv.set(wrap_node(scope, node_id).into());
    } else {
        rv.set(v8::null(scope).into());
    }
}

fn append_child_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Some(parent_id) = get_number_property(scope, args.this(), NODE_ID_KEY) else {
        return;
    };
    let Ok(child) = v8::Local::<v8::Object>::try_from(args.get(0)) else {
        return;
    };
    let Some(child_id) = get_number_property(scope, child, NODE_ID_KEY) else {
        return;
    };

    if get_context_data(scope)
        .dom_tree
        .write()
        .append_child(parent_id, child_id)
    {
        rv.set(child.into());
    }
}

fn remove_child_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Some(parent_id) = get_number_property(scope, args.this(), NODE_ID_KEY) else {
        return;
    };
    let Ok(child) = v8::Local::<v8::Object>::try_from(args.get(0)) else {
        return;
    };
    let Some(child_id) = get_number_property(scope, child, NODE_ID_KEY) else {
        return;
    };

    if get_context_data(scope)
        .dom_tree
        .write()
        .remove_child(parent_id, child_id)
    {
        rv.set(child.into());
    }
}

fn set_attribute_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(node_id) = get_number_property(scope, args.this(), NODE_ID_KEY) else {
        return;
    };
    let Some(name) = value_to_string(scope, args.get(0)) else {
        return;
    };
    let value = value_to_string(scope, args.get(1)).unwrap_or_default();
    get_context_data(scope)
        .dom_tree
        .write()
        .set_attribute(node_id, &name, &value);
}

fn get_attribute_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Some(node_id) = get_number_property(scope, args.this(), NODE_ID_KEY) else {
        rv.set(v8::null(scope).into());
        return;
    };
    let Some(name) = value_to_string(scope, args.get(0)) else {
        rv.set(v8::null(scope).into());
        return;
    };

    let value = {
        let dom_tree = get_context_data(scope).dom_tree.read();
        dom_tree
            .get_node(node_id)
            .and_then(|node| node.get_attribute(&name).map(str::to_owned))
    };

    if let Some(value) = value.and_then(|value| v8::String::new(scope, &value)) {
        rv.set(value.into());
    } else {
        rv.set(v8::null(scope).into());
    }
}

fn add_event_listener_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(node_id) = get_number_property(scope, args.this(), NODE_ID_KEY) else {
        return;
    };
    let Some(event_type) = value_to_string(scope, args.get(0)) else {
        return;
    };
    let Ok(callback) = v8::Local::<v8::Function>::try_from(args.get(1)) else {
        return;
    };

    get_context_data(scope)
        .event_listeners
        .write()
        .entry((node_id, event_type))
        .or_default()
        .push(v8::Global::new(scope, callback));
}

fn get_storage<'a>(
    scope: &mut v8::HandleScope,
    this: v8::Local<v8::Object>,
) -> &'a Arc<RwLock<StorageApi>> {
    let data = get_context_data(scope);
    let storage_type = get_number_property(scope, this, STORAGE_TYPE_KEY).unwrap_or(0);
    if storage_type == 0 {
        &data.local_storage
    } else {
        &data.session_storage
    }
}

fn storage_get_item(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let key = value_to_string(scope, args.get(0)).unwrap_or_default();
    let value = {
        let storage = get_storage(scope, args.this());
        storage.read().get_item(&key).map(str::to_owned)
    };
    if let Some(value) = value {
        if let Some(v8_str) = v8::String::new(scope, &value) {
            rv.set(v8_str.into());
        }
    } else {
        rv.set(v8::null(scope).into());
    }
}

fn storage_set_item(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let key = value_to_string(scope, args.get(0)).unwrap_or_default();
    let value = value_to_string(scope, args.get(1)).unwrap_or_default();
    {
        let storage = get_storage(scope, args.this());
        let _ = storage.write().set_item(&key, &value);
    }
}

fn storage_remove_item(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let key = value_to_string(scope, args.get(0)).unwrap_or_default();
    {
        let storage = get_storage(scope, args.this());
        storage.write().remove_item(&key);
    }
}

fn storage_clear(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    {
        let storage = get_storage(scope, args.this());
        storage.write().clear();
    }
}

// --- MutationObserver ---

fn mutation_observer_constructor(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Ok(callback) = v8::Local::<v8::Function>::try_from(args.get(0)) else {
        return;
    };
    let id = NEXT_OBSERVER_ID.fetch_add(1, Ordering::Relaxed);
    let object = v8::Object::new(scope);
    set_number_property(scope, object, OBSERVER_ID_KEY, id);

    get_context_data(scope).mutation_observers.write().insert(
        id,
        MutationObserverState {
            id,
            callback: v8::Global::new(scope, callback),
            targets: Vec::new(),
            records: Vec::new(),
        },
    );

    let observe_fn = v8::Function::new(scope, mutation_observer_observe).expect("observe");
    set_property(scope, object, "observe", observe_fn.into());
    let disconnect_fn = v8::Function::new(scope, mutation_observer_disconnect).expect("disconnect");
    set_property(scope, object, "disconnect", disconnect_fn.into());
    let take_records_fn =
        v8::Function::new(scope, mutation_observer_take_records).expect("takeRecords");
    set_property(scope, object, "takeRecords", take_records_fn.into());

    rv.set(object.into());
}

fn mutation_observer_observe(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(observer_id) = get_number_property(scope, args.this(), OBSERVER_ID_KEY) else {
        return;
    };
    let Ok(target) = v8::Local::<v8::Object>::try_from(args.get(0)) else {
        return;
    };
    let Some(target_id) = get_number_property(scope, target, NODE_ID_KEY) else {
        return;
    };
    if let Some(observer) = get_context_data(scope)
        .mutation_observers
        .write()
        .get_mut(&observer_id)
    {
        if !observer.targets.contains(&target_id) {
            observer.targets.push(target_id);
        }
    }
}

fn mutation_observer_disconnect(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(observer_id) = get_number_property(scope, args.this(), OBSERVER_ID_KEY) else {
        return;
    };
    get_context_data(scope)
        .mutation_observers
        .write()
        .remove(&observer_id);
}

fn mutation_observer_take_records(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Some(observer_id) = get_number_property(scope, args.this(), OBSERVER_ID_KEY) else {
        rv.set(v8::Array::new(scope, 0).into());
        return;
    };
    let records = {
        let mut observers = get_context_data(scope).mutation_observers.write();
        observers
            .get_mut(&observer_id)
            .map(|o| std::mem::take(&mut o.records))
            .unwrap_or_default()
    };
    let array = v8::Array::new(scope, records.len() as i32);
    for (i, record) in records.iter().enumerate() {
        let obj = v8::Object::new(scope);
        if let Some(t) = v8::String::new(scope, &record.record_type) {
            set_property(scope, obj, "type", t.into());
        }
        let target = wrap_node(scope, record.target_id);
        set_property(scope, obj, "target", target.into());
        let _ = array.set_index(scope, i as u32, obj.into());
    }
    rv.set(array.into());
}

// --- IntersectionObserver ---

fn intersection_observer_constructor(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Ok(callback) = v8::Local::<v8::Function>::try_from(args.get(0)) else {
        return;
    };
    let id = NEXT_OBSERVER_ID.fetch_add(1, Ordering::Relaxed);
    let object = v8::Object::new(scope);
    set_number_property(scope, object, OBSERVER_ID_KEY, id);

    get_context_data(scope)
        .intersection_observers
        .write()
        .insert(
            id,
            IntersectionObserverState {
                id,
                callback: v8::Global::new(scope, callback),
                targets: Vec::new(),
            },
        );

    let observe_fn = v8::Function::new(scope, intersection_observer_observe).expect("observe");
    set_property(scope, object, "observe", observe_fn.into());
    let unobserve_fn = v8::Function::new(scope, intersection_observer_unobserve).expect("unobserve");
    set_property(scope, object, "unobserve", unobserve_fn.into());
    let disconnect_fn =
        v8::Function::new(scope, intersection_observer_disconnect).expect("disconnect");
    set_property(scope, object, "disconnect", disconnect_fn.into());

    rv.set(object.into());
}

fn intersection_observer_observe(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(observer_id) = get_number_property(scope, args.this(), OBSERVER_ID_KEY) else {
        return;
    };
    let Ok(target) = v8::Local::<v8::Object>::try_from(args.get(0)) else {
        return;
    };
    let Some(target_id) = get_number_property(scope, target, NODE_ID_KEY) else {
        return;
    };
    if let Some(observer) = get_context_data(scope)
        .intersection_observers
        .write()
        .get_mut(&observer_id)
    {
        if !observer.targets.contains(&target_id) {
            observer.targets.push(target_id);
        }
    }
}

fn intersection_observer_unobserve(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(observer_id) = get_number_property(scope, args.this(), OBSERVER_ID_KEY) else {
        return;
    };
    let Ok(target) = v8::Local::<v8::Object>::try_from(args.get(0)) else {
        return;
    };
    let Some(target_id) = get_number_property(scope, target, NODE_ID_KEY) else {
        return;
    };
    if let Some(observer) = get_context_data(scope)
        .intersection_observers
        .write()
        .get_mut(&observer_id)
    {
        observer.targets.retain(|id| *id != target_id);
    }
}

fn intersection_observer_disconnect(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(observer_id) = get_number_property(scope, args.this(), OBSERVER_ID_KEY) else {
        return;
    };
    get_context_data(scope)
        .intersection_observers
        .write()
        .remove(&observer_id);
}

// --- Canvas 2D ---

fn canvas_get_context_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let context_type = value_to_string(scope, args.get(0)).unwrap_or_default();
    if context_type != "2d" {
        rv.set(v8::null(scope).into());
        return;
    }
    let Some(node_id) = get_number_property(scope, args.this(), NODE_ID_KEY) else {
        rv.set(v8::null(scope).into());
        return;
    };
    let ctx_id = NEXT_CANVAS_CTX_ID.fetch_add(1, Ordering::Relaxed);
    get_context_data(scope).canvas_contexts.write().insert(
        ctx_id,
        Canvas2DState {
            id: ctx_id,
            node_id,
            fill_style: "#000000".to_string(),
            stroke_style: "#000000".to_string(),
            path: Vec::new(),
        },
    );

    let ctx = v8::Object::new(scope);
    set_number_property(scope, ctx, CANVAS_CTX_ID_KEY, ctx_id);

    let fill_rect = v8::Function::new(scope, canvas_fill_rect).expect("fillRect");
    set_property(scope, ctx, "fillRect", fill_rect.into());
    let clear_rect = v8::Function::new(scope, canvas_clear_rect).expect("clearRect");
    set_property(scope, ctx, "clearRect", clear_rect.into());
    let begin_path = v8::Function::new(scope, canvas_begin_path).expect("beginPath");
    set_property(scope, ctx, "beginPath", begin_path.into());
    let line_to = v8::Function::new(scope, canvas_line_to).expect("lineTo");
    set_property(scope, ctx, "lineTo", line_to.into());
    let arc = v8::Function::new(scope, canvas_arc).expect("arc");
    set_property(scope, ctx, "arc", arc.into());
    let fill = v8::Function::new(scope, canvas_fill).expect("fill");
    set_property(scope, ctx, "fill", fill.into());
    let stroke = v8::Function::new(scope, canvas_stroke).expect("stroke");
    set_property(scope, ctx, "stroke", stroke.into());

    rv.set(ctx.into());
}

fn canvas_ctx_id(scope: &mut v8::HandleScope, this: v8::Local<v8::Object>) -> Option<u64> {
    get_number_property(scope, this, CANVAS_CTX_ID_KEY)
}

fn canvas_fill_rect(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(ctx_id) = canvas_ctx_id(scope, args.this()) else {
        return;
    };
    let _x = args.get(0).number_value(scope).unwrap_or(0.0);
    let _y = args.get(1).number_value(scope).unwrap_or(0.0);
    let _w = args.get(2).number_value(scope).unwrap_or(0.0);
    let _h = args.get(3).number_value(scope).unwrap_or(0.0);
    if let Some(ctx) = get_context_data(scope).canvas_contexts.write().get_mut(&ctx_id) {
        ctx.path.clear();
        let _ = (ctx.node_id, _x, _y, _w, _h);
    }
}

fn canvas_clear_rect(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(ctx_id) = canvas_ctx_id(scope, args.this()) else {
        return;
    };
    if let Some(ctx) = get_context_data(scope).canvas_contexts.write().get_mut(&ctx_id) {
        ctx.path.clear();
    }
    let _ = args;
}

fn canvas_begin_path(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(ctx_id) = canvas_ctx_id(scope, args.this()) else {
        return;
    };
    if let Some(ctx) = get_context_data(scope).canvas_contexts.write().get_mut(&ctx_id) {
        ctx.path.clear();
    }
}

fn canvas_line_to(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(ctx_id) = canvas_ctx_id(scope, args.this()) else {
        return;
    };
    let x = args.get(0).number_value(scope).unwrap_or(0.0);
    let y = args.get(1).number_value(scope).unwrap_or(0.0);
    if let Some(ctx) = get_context_data(scope).canvas_contexts.write().get_mut(&ctx_id) {
        ctx.path.push(CanvasPathOp::LineTo(x, y));
    }
}

fn canvas_arc(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(ctx_id) = canvas_ctx_id(scope, args.this()) else {
        return;
    };
    let x = args.get(0).number_value(scope).unwrap_or(0.0);
    let y = args.get(1).number_value(scope).unwrap_or(0.0);
    let r = args.get(2).number_value(scope).unwrap_or(0.0);
    let start = args.get(3).number_value(scope).unwrap_or(0.0);
    let end = args.get(4).number_value(scope).unwrap_or(0.0);
    let ccw = args.get(5).boolean_value(scope);
    if let Some(ctx) = get_context_data(scope).canvas_contexts.write().get_mut(&ctx_id) {
        ctx.path.push(CanvasPathOp::Arc(x, y, r, start, end, ccw));
    }
}

fn canvas_fill(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let _ = args;
    let Some(ctx_id) = canvas_ctx_id(scope, args.this()) else {
        return;
    };
    if let Some(ctx) = get_context_data(scope).canvas_contexts.write().get_mut(&ctx_id) {
        ctx.path.clear();
    }
}

fn canvas_stroke(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let _ = args;
    let Some(ctx_id) = canvas_ctx_id(scope, args.this()) else {
        return;
    };
    if let Some(ctx) = get_context_data(scope).canvas_contexts.write().get_mut(&ctx_id) {
        ctx.path.clear();
    }
}

fn input_value_getter(
    scope: &mut v8::HandleScope,
    _name: v8::Local<v8::Name>,
    args: v8::PropertyCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Some(node_id) = get_number_property(scope, args.this(), NODE_ID_KEY) else {
        return;
    };
    let value = {
        let dom_tree = get_context_data(scope).dom_tree.read();
        dom_tree
            .get_node(node_id)
            .and_then(|n| n.get_attribute("value"))
            .unwrap_or("")
            .to_string()
    };
    if let Some(v8_str) = v8::String::new(scope, &value) {
        rv.set(v8_str.into());
    }
}

fn input_value_setter(
    scope: &mut v8::HandleScope,
    _name: v8::Local<v8::Name>,
    value: v8::Local<v8::Value>,
    args: v8::PropertyCallbackArguments,
) {
    let Some(node_id) = get_number_property(scope, args.this(), NODE_ID_KEY) else {
        return;
    };
    let text = value_to_string(scope, value).unwrap_or_default();
    get_context_data(scope)
        .dom_tree
        .write()
        .set_attribute(node_id, "value", &text);
}

// --- IndexedDB ---

fn indexeddb_open_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let db_name = value_to_string(scope, args.get(0)).unwrap_or_default();
    let version = args.get(1).integer_value(scope).unwrap_or(1) as u32;

    let request = v8::Object::new(scope);
    if let Some(name) = v8::String::new(scope, &db_name) {
        set_property(scope, request, "name", name.into());
    }

    let db_result: Result<DatabaseMetadata, crate::storage::StorageError> = {
        let idb = get_context_data(scope).indexeddb.read();
        match idb.get_metadata(&db_name) {
            Ok(mut meta) => {
                if version > meta.version {
                    meta.version = version;
                    drop(idb);
                    let _ = get_context_data(scope)
                        .indexeddb
                        .write()
                        .set_metadata(&db_name, meta.clone());
                }
                Ok(meta)
            }
            Err(_) => {
                let meta = DatabaseMetadata::new(&db_name, version);
                drop(idb);
                let _ = get_context_data(scope)
                    .indexeddb
                    .write()
                    .set_metadata(&db_name, meta.clone());
                Ok(meta)
            }
        }
    };

    let db_obj = v8::Object::new(scope);
    if let Ok(meta) = db_result {
        set_number_property(scope, db_obj, "version", meta.version as u64);
        if let Some(name) = v8::String::new(scope, &meta.name) {
            set_property(scope, db_obj, "name", name.into());
        }
        if let Some(db_name_val) = v8::String::new(scope, &db_name) {
            set_property(scope, db_obj, IDB_DB_NAME_KEY, db_name_val.into());
        }

        let transaction_fn = v8::Function::new(scope, idb_transaction_callback).expect("transaction");
        set_property(scope, db_obj, "transaction", transaction_fn.into());
        let create_store_fn =
            v8::Function::new(scope, idb_create_object_store_callback).expect("createObjectStore");
        set_property(scope, db_obj, "createObjectStore", create_store_fn.into());

        set_property(scope, request, "result", db_obj.into());
    }

    rv.set(request.into());
}

fn get_object_string_property(
    scope: &mut v8::HandleScope,
    object: v8::Local<v8::Object>,
    name: &str,
) -> Option<String> {
    let key = v8::String::new(scope, name)?;
    let value = object.get(scope, key.into())?;
    value_to_string(scope, value)
}

fn set_object_string_property<'s>(
    scope: &mut v8::HandleScope<'s>,
    object: v8::Local<v8::Object>,
    name: &str,
    value: &str,
) {
    if let Some(v8_str) = v8::String::new(scope, value) {
        set_property(scope, object, name, v8_str.into());
    }
}

fn idb_transaction_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Ok(db_obj) = v8::Local::<v8::Object>::try_from(args.this()) else {
        return;
    };
    let db_name = get_object_string_property(scope, db_obj, IDB_DB_NAME_KEY).unwrap_or_default();
    let store_name = value_to_string(scope, args.get(0)).unwrap_or_default();

    let tx = v8::Object::new(scope);
    set_object_string_property(scope, tx, IDB_DB_NAME_KEY, &db_name);
    set_object_string_property(scope, tx, IDB_STORE_NAME_KEY, &store_name);

    let object_store_fn = v8::Function::new(scope, idb_object_store_callback).expect("objectStore");
    set_property(scope, tx, "objectStore", object_store_fn.into());

    rv.set(tx.into());
}

fn idb_object_store_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Ok(tx) = v8::Local::<v8::Object>::try_from(args.this()) else {
        return;
    };
    let (db_name, store_name) = idb_store_names(scope, tx);
    let store = v8::Object::new(scope);
    set_object_string_property(scope, store, IDB_DB_NAME_KEY, &db_name);
    set_object_string_property(scope, store, IDB_STORE_NAME_KEY, &store_name);
    let put_fn = v8::Function::new(scope, idb_put_callback).expect("put");
    set_property(scope, store, "put", put_fn.into());
    let get_fn = v8::Function::new(scope, idb_get_callback).expect("get");
    set_property(scope, store, "get", get_fn.into());
    let delete_fn = v8::Function::new(scope, idb_delete_callback).expect("delete");
    set_property(scope, store, "delete", delete_fn.into());
    let get_all_fn = v8::Function::new(scope, idb_get_all_callback).expect("getAll");
    set_property(scope, store, "getAll", get_all_fn.into());
    rv.set(store.into());
}

fn idb_create_object_store_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let Ok(db_obj) = v8::Local::<v8::Object>::try_from(args.this()) else {
        return;
    };
    let db_name = get_object_string_property(scope, db_obj, IDB_DB_NAME_KEY).unwrap_or_default();
    let store_name = value_to_string(scope, args.get(0)).unwrap_or_default();
    let auto_increment = args.get(1).is_object() || args.get(1).is_boolean();

    let store_meta = ObjectStoreMetadata {
        name: store_name.clone(),
        key_path: None,
        auto_increment,
        current_auto_id: 0,
        indexes: Vec::new(),
    };
    let _ = get_context_data(scope)
        .indexeddb
        .write()
        .create_object_store(&db_name, store_meta);

    let store = v8::Object::new(scope);
    if let Some(name) = v8::String::new(scope, &store_name) {
        set_property(scope, store, "name", name.into());
    }
    rv.set(store.into());
}

fn js_value_to_json(scope: &mut v8::HandleScope, value: v8::Local<v8::Value>) -> JsonValue {
    if value.is_undefined() || value.is_null() {
        JsonValue::Null
    } else if value.is_boolean() {
        JsonValue::Bool(value.boolean_value(scope))
    } else if value.is_number() {
        JsonValue::Number(
            serde_json::Number::from_f64(value.number_value(scope).unwrap_or(0.0))
                .unwrap_or_else(|| serde_json::Number::from(0)),
        )
    } else if value.is_string() {
        JsonValue::String(value_to_string(scope, value).unwrap_or_default())
    } else {
        JsonValue::String(value_to_string(scope, value).unwrap_or_default())
    }
}

fn json_to_v8<'s>(
    scope: &mut v8::HandleScope<'s>,
    value: &JsonValue,
) -> v8::Local<'s, v8::Value> {
    match value {
        JsonValue::Null => v8::null(scope).into(),
        JsonValue::Bool(b) => v8::Boolean::new(scope, *b).into(),
        JsonValue::Number(n) => v8::Number::new(scope, n.as_f64().unwrap_or(0.0)).into(),
        JsonValue::String(s) => v8::String::new(scope, s)
            .map(|v| v.into())
            .unwrap_or_else(|| v8::undefined(scope).into()),
        JsonValue::Array(arr) => {
            let array = v8::Array::new(scope, arr.len() as i32);
            for (i, item) in arr.iter().enumerate() {
                let element = json_to_v8(scope, item);
                let _ = array.set_index(scope, i as u32, element);
            }
            array.into()
        }
        JsonValue::Object(map) => {
            let obj = v8::Object::new(scope);
            for (k, v) in map {
                let element = json_to_v8(scope, v);
                set_property(scope, obj, k, element);
            }
            obj.into()
        }
    }
}

fn idb_store_names(scope: &mut v8::HandleScope, this: v8::Local<v8::Object>) -> (String, String) {
    let db_name = get_object_string_property(scope, this, IDB_DB_NAME_KEY).unwrap_or_default();
    let store_name = get_object_string_property(scope, this, IDB_STORE_NAME_KEY).unwrap_or_default();
    (db_name, store_name)
}

fn idb_put_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let (db_name, store_name) = idb_store_names(scope, args.this());
    let value = js_value_to_json(scope, args.get(0));
    let key = if args.length() > 1 {
        js_value_to_json(scope, args.get(1))
    } else {
        match get_context_data(scope)
            .indexeddb
            .write()
            .next_auto_id(&db_name, &store_name)
        {
            Ok(id) => JsonValue::Number(id.into()),
            Err(_) => JsonValue::String(value_to_string(scope, args.get(0)).unwrap_or_default()),
        }
    };
    let result = get_context_data(scope)
        .indexeddb
        .write()
        .put(&db_name, &store_name, key.clone(), value);
    if result.is_ok() {
        rv.set(json_to_v8(scope, &key));
    }
}

fn idb_get_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let (db_name, store_name) = idb_store_names(scope, args.this());
    let key = js_value_to_json(scope, args.get(0));
    match get_context_data(scope)
        .indexeddb
        .read()
        .get(&db_name, &store_name, &key)
    {
        Ok(Some(value)) => rv.set(json_to_v8(scope, &value)),
        _ => rv.set(v8::undefined(scope).into()),
    }
}

fn idb_delete_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let (db_name, store_name) = idb_store_names(scope, args.this());
    let key = js_value_to_json(scope, args.get(0));
    let _ = get_context_data(scope)
        .indexeddb
        .write()
        .delete(&db_name, &store_name, &key);
}

fn idb_get_all_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let (db_name, store_name) = idb_store_names(scope, args.this());
    let range = KeyRange::bound(JsonValue::Null, JsonValue::String("\u{10FFFF}".into()), false, false);
    let results = get_context_data(scope)
        .indexeddb
        .read()
        .query_range(&db_name, &store_name, &range)
        .unwrap_or_default();
    let array = v8::Array::new(scope, results.len() as i32);
    for (i, (_, value)) in results.iter().enumerate() {
        let element = json_to_v8(scope, value);
        let _ = array.set_index(scope, i as u32, element);
    }
    rv.set(array.into());
}

// --- WebSocket ---

fn websocket_constructor(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let url = value_to_string(scope, args.get(0)).unwrap_or_default();
    let conn = get_context_data(scope).websocket_manager.write().create(&url);
    let _ = conn.connect();
    let id = conn.id();

    let ws = v8::Object::new(scope);
    set_number_property(scope, ws, WEBSOCKET_ID_KEY, id);
    if let Some(url_str) = v8::String::new(scope, &url) {
        set_property(scope, ws, "url", url_str.into());
    }
    let ready_state = websocket_ready_state(&conn);
    let ready_state_value = v8::Integer::new(scope, ready_state);
    set_property(scope, ws, "readyState", ready_state_value.into());

    let send_fn = v8::Function::new(scope, websocket_send_callback).expect("send");
    set_property(scope, ws, "send", send_fn.into());
    let close_fn = v8::Function::new(scope, websocket_close_callback).expect("close");
    set_property(scope, ws, "close", close_fn.into());

    rv.set(ws.into());
}

fn websocket_ready_state(conn: &WebSocketConnection) -> i32 {
    match conn.state() {
        WebSocketState::Connecting => 0,
        WebSocketState::Open => 1,
        WebSocketState::Closing => 2,
        WebSocketState::Closed => 3,
    }
}

fn websocket_by_id(scope: &mut v8::HandleScope, id: u64) -> Option<WebSocketConnection> {
    get_context_data(scope).websocket_manager.read().get(id)
}

fn websocket_send_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(id) = get_number_property(scope, args.this(), WEBSOCKET_ID_KEY) else {
        return;
    };
    let Some(conn) = websocket_by_id(scope, id) else {
        return;
    };
    let text = value_to_string(scope, args.get(0)).unwrap_or_default();
    let _ = conn.send_frame(WebSocketFrame::Text(text));
}

fn websocket_close_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(id) = get_number_property(scope, args.this(), WEBSOCKET_ID_KEY) else {
        return;
    };
    let Some(conn) = websocket_by_id(scope, id) else {
        return;
    };
    let code = args.get(0).integer_value(scope).map(|c| c as u16);
    let reason = value_to_string(scope, args.get(1));
    let _ = conn.close(code, reason);
}

// --- Worker ---

fn worker_constructor(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let script_url = value_to_string(scope, args.get(0)).unwrap_or_default();
    let id = NEXT_WORKER_ID.fetch_add(1, Ordering::Relaxed);
    let script = format!("// worker from {script_url}");
    match V8Worker::spawn(id, &script) {
        Ok(worker) => {
            let worker_obj = v8::Object::new(scope);
            set_number_property(scope, worker_obj, WORKER_ID_KEY, id);

            let post_message_fn = v8::Function::new(scope, worker_post_message_callback).expect("postMessage");
            set_property(scope, worker_obj, "postMessage", post_message_fn.into());
            let terminate_fn = v8::Function::new(scope, worker_terminate_callback).expect("terminate");
            set_property(scope, worker_obj, "terminate", terminate_fn.into());

            get_context_data(scope).workers.write().insert(id, worker);
            rv.set(worker_obj.into());
        }
        Err(_) => rv.set(v8::null(scope).into()),
    }
}

fn worker_post_message_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(id) = get_number_property(scope, args.this(), WORKER_ID_KEY) else {
        return;
    };
    let message = WorkerMessage::Text(value_to_string(scope, args.get(0)).unwrap_or_default());
    if let Some(worker) = get_context_data(scope).workers.read().get(&id) {
        let _ = worker.post_message(message);
    }
}

fn worker_terminate_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let Some(id) = get_number_property(scope, args.this(), WORKER_ID_KEY) else {
        return;
    };
    if let Some(mut worker) = get_context_data(scope).workers.write().remove(&id) {
        worker.terminate();
    }
}

pub(crate) fn dispatch_event(scope: &mut v8::HandleScope, event: &DomEvent) -> usize {
    let callbacks = {
        let data = get_context_data(scope);
        data.event_listeners
            .read()
            .get(&(event.target_id, event.event_type.clone()))
            .cloned()
            .unwrap_or_default()
    };

    if callbacks.is_empty() {
        return 0;
    }

    let event_object = create_event_object(scope, event);
    let receiver = v8::undefined(scope).into();

    for callback in &callbacks {
        let callback = v8::Local::new(scope, callback);
        let _ = callback.call(scope, receiver, &[event_object.into()]);
    }

    callbacks.len()
}

fn create_event_object<'s>(
    scope: &mut v8::HandleScope<'s>,
    event: &DomEvent,
) -> v8::Local<'s, v8::Object> {
    let object = v8::Object::new(scope);

    if let Some(event_type) = v8::String::new(scope, &event.event_type) {
        set_property(scope, object, "type", event_type.into());
    }

    let target = wrap_node(scope, event.target_id);
    set_property(scope, object, "target", target.into());

    if let Some(client_x) = event.client_x {
        let value = v8::Number::new(scope, client_x.into());
        set_property(scope, object, "clientX", value.into());
    }

    if let Some(client_y) = event.client_y {
        let value = v8::Number::new(scope, client_y.into());
        set_property(scope, object, "clientY", value.into());
    }

    if let Some(button) = event.button {
        let value = v8::Integer::new(scope, button.into());
        set_property(scope, object, "button", value.into());
    }

    if let Some(key) = &event.key {
        if let Some(key) = v8::String::new(scope, key) {
            set_property(scope, object, "key", key.into());
        }
    }

    object
}

pub(crate) fn get_context_data(scope: &mut v8::HandleScope) -> &'static V8ContextData {
    let ptr = context_data_ptr(scope).expect("V8 context data should be installed");
    // SAFETY: `ptr` is owned by the current V8 context and freed by `take_context_data`.
    unsafe { ptr.as_ref() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networking::WebSocketManager;
    use crate::servo_embed::dom::DomTree;
    use crate::servo_embed::web_apis::{ConsoleApi, StorageApi, TimerManager};
    use crate::storage::IndexedDb;
    use parking_lot::RwLock;
    use std::sync::Arc;

    fn create_test_context_data() -> V8ContextData {
        let dom_tree = Arc::new(RwLock::new(DomTree::new()));
        let console_api = Arc::new(RwLock::new(ConsoleApi::new()));
        let timer_manager = Arc::new(RwLock::new(TimerManager::new()));
        let local_storage = Arc::new(RwLock::new(StorageApi::new(1024)));
        let session_storage = Arc::new(RwLock::new(StorageApi::new(1024)));
        let indexeddb = Arc::new(RwLock::new(IndexedDb::ephemeral()));
        let websocket_manager = Arc::new(RwLock::new(WebSocketManager::new()));

        V8ContextData::new(
            dom_tree,
            console_api,
            timer_manager,
            local_storage,
            session_storage,
            indexeddb,
            websocket_manager,
        )
    }

    #[test]
    fn test_v8_context_data_new() {
        let dom_tree = Arc::new(RwLock::new(DomTree::new()));
        let console_api = Arc::new(RwLock::new(ConsoleApi::new()));
        let timer_manager = Arc::new(RwLock::new(TimerManager::new()));
        let local_storage = Arc::new(RwLock::new(StorageApi::new(1024 * 1024)));
        let session_storage = Arc::new(RwLock::new(StorageApi::new(1024 * 1024)));

        let indexeddb = Arc::new(RwLock::new(IndexedDb::ephemeral()));
        let websocket_manager = Arc::new(RwLock::new(WebSocketManager::new()));

        let context_data = V8ContextData::new(
            Arc::clone(&dom_tree),
            Arc::clone(&console_api),
            Arc::clone(&timer_manager),
            Arc::clone(&local_storage),
            Arc::clone(&session_storage),
            indexeddb,
            websocket_manager,
        );

        assert!(Arc::ptr_eq(&context_data.dom_tree, &dom_tree));
        assert!(Arc::ptr_eq(&context_data.console_api, &console_api));
        assert!(Arc::ptr_eq(&context_data.timer_manager, &timer_manager));
        assert!(Arc::ptr_eq(&context_data.local_storage, &local_storage));
        assert!(Arc::ptr_eq(&context_data.session_storage, &session_storage));
        assert!(context_data.timer_callbacks.read().is_empty());
        assert!(context_data.event_listeners.read().is_empty());
    }

    #[test]
    fn test_take_context_data() {
        crate::js::engine::init_v8();
        let mut isolate = v8::Isolate::new(v8::CreateParams::default());
        let mut handle_scope = v8::HandleScope::new(&mut isolate);

        let dom_tree = Arc::new(RwLock::new(DomTree::new()));
        let console_api = Arc::new(RwLock::new(ConsoleApi::new()));
        let timer_manager = Arc::new(RwLock::new(TimerManager::new()));
        let local_storage = Arc::new(RwLock::new(StorageApi::new(1024)));
        let session_storage = Arc::new(RwLock::new(StorageApi::new(1024)));

        let indexeddb = Arc::new(RwLock::new(IndexedDb::ephemeral()));
        let websocket_manager = Arc::new(RwLock::new(WebSocketManager::new()));

        let data = V8ContextData::new(
            dom_tree.clone(),
            console_api.clone(),
            timer_manager.clone(),
            local_storage.clone(),
            session_storage.clone(),
            indexeddb,
            websocket_manager,
        );

        let context = initialize_context(&mut handle_scope, data);
        let mut context_scope = v8::ContextScope::new(&mut handle_scope, context);

        let taken_data = take_context_data(&mut context_scope);
        assert!(taken_data.is_some());
        assert_eq!(Arc::strong_count(&taken_data.unwrap().dom_tree), 2);

        let taken_again = take_context_data(&mut context_scope);
        assert!(taken_again.is_none());
    }

    #[test]
    fn test_get_context_data_success() {
        crate::js::engine::init_v8();
        let mut isolate = v8::Isolate::new(v8::CreateParams::default());
        let scope = &mut v8::HandleScope::new(&mut isolate);

        let context_data = create_test_context_data();
        let context = initialize_context(scope, context_data);
        let scope = &mut v8::ContextScope::new(scope, context);

        let retrieved_data = get_context_data(scope);
        assert!(retrieved_data.dom_tree.read().get_node(0).is_none());

        let _ = take_context_data(scope);
    }

    #[test]
    #[should_panic(expected = "V8 context data should be installed")]
    fn test_get_context_data_missing() {
        crate::js::engine::init_v8();
        let mut isolate = v8::Isolate::new(v8::CreateParams::default());
        let scope = &mut v8::HandleScope::new(&mut isolate);

        let context = v8::Context::new(scope);
        let scope = &mut v8::ContextScope::new(scope, context);

        let _ = get_context_data(scope);
    }
}
