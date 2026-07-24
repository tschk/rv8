//! V8 Web Worker implementation
//!
//! Spawns dedicated OS threads with independent V8 isolates and provides
//! thread-safe message passing between the main thread and worker isolates.

use crate::js::engine::init_v8;
use parking_lot::RwLock;
use rusty_v8 as v8;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const WORKER_DATA_KEY: &str = "__rv8_worker_data";

/// Message passed between the main thread and a worker isolate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerMessage {
    Text(String),
    Binary(Vec<u8>),
}

enum WorkerCommand {
    Message(WorkerMessage),
    Terminate,
}

struct WorkerContextData {
    outbox: Sender<WorkerMessage>,
    onmessage: RwLock<Option<v8::Global<v8::Function>>>,
}

/// A dedicated V8 worker running on its own OS thread.
pub struct V8Worker {
    id: u64,
    to_worker: Sender<WorkerCommand>,
    from_worker: Receiver<WorkerMessage>,
    terminated: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl V8Worker {
    /// Spawn a new worker with the given id and initial script.
    pub fn spawn(id: u64, script: &str) -> Result<Self, String> {
        init_v8();

        let (to_worker, from_main) = mpsc::channel();
        let (to_main, from_worker) = mpsc::channel();
        let terminated = Arc::new(AtomicBool::new(false));
        let terminated_worker = Arc::clone(&terminated);
        let script = script.to_string();

        let thread = thread::Builder::new()
            .name(format!("rv8-worker-{id}"))
            .spawn(move || {
                if let Err(err) = run_worker(id, &script, from_main, to_main, terminated_worker) {
                    log::error!("Worker {id} failed: {err}");
                }
            })
            .map_err(|err| format!("Failed to spawn worker thread: {err}"))?;

        Ok(V8Worker {
            id,
            to_worker,
            from_worker,
            terminated,
            thread: Some(thread),
        })
    }

    /// Return the worker id.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Post a message to the worker isolate.
    pub fn post_message(&self, message: WorkerMessage) -> Result<(), String> {
        if self.terminated.load(Ordering::Relaxed) {
            return Err("Worker has been terminated".to_string());
        }
        self.to_worker
            .send(WorkerCommand::Message(message))
            .map_err(|_| "Worker thread is not running".to_string())
    }

    /// Poll for messages sent from the worker via `postMessage`.
    pub fn poll_messages(&self) -> Vec<WorkerMessage> {
        let mut messages = Vec::new();
        while let Ok(message) = self.from_worker.try_recv() {
            messages.push(message);
        }
        messages
    }

    /// Terminate the worker thread and its isolate.
    pub fn terminate(&mut self) {
        if self.terminated.swap(true, Ordering::Relaxed) {
            return;
        }
        let _ = self.to_worker.send(WorkerCommand::Terminate);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for V8Worker {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn run_worker(
    id: u64,
    script: &str,
    from_main: Receiver<WorkerCommand>,
    to_main: Sender<WorkerMessage>,
    terminated: Arc<AtomicBool>,
) -> Result<(), String> {
    init_v8();

    let mut isolate = v8::Isolate::new(v8::CreateParams::default());
    let context_global = {
        let handle_scope = &mut v8::HandleScope::new(&mut isolate);
        let context = setup_worker_context(handle_scope, to_main.clone())?;
        v8::Global::new(handle_scope, context)
    };

    {
        let handle_scope = &mut v8::HandleScope::new(&mut isolate);
        let context = v8::Local::new(handle_scope, &context_global);
        let scope = &mut v8::ContextScope::new(handle_scope, context);
        execute_script(scope, script)?;
    }

    loop {
        if terminated.load(Ordering::Relaxed) {
            break;
        }

        match from_main.recv_timeout(Duration::from_millis(50)) {
            Ok(WorkerCommand::Message(message)) => {
                {
                    let handle_scope = &mut v8::HandleScope::new(&mut isolate);
                    let context = v8::Local::new(handle_scope, &context_global);
                    let scope = &mut v8::ContextScope::new(handle_scope, context);
                    dispatch_onmessage(scope, message)?;
                }
                isolate.perform_microtask_checkpoint();
            }
            Ok(WorkerCommand::Terminate) => break,
            Err(RecvTimeoutError::Timeout) => {
                isolate.perform_microtask_checkpoint();
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    {
        let handle_scope = &mut v8::HandleScope::new(&mut isolate);
        let context = v8::Local::new(handle_scope, &context_global);
        let scope = &mut v8::ContextScope::new(handle_scope, context);
        let _ = take_worker_data(scope);
    }

    log::debug!("Worker {id} terminated");
    Ok(())
}

fn setup_worker_context<'s>(
    scope: &mut v8::HandleScope<'s, ()>,
    outbox: Sender<WorkerMessage>,
) -> Result<v8::Local<'s, v8::Context>, String> {
    let context = v8::Context::new(scope);
    let scope = &mut v8::ContextScope::new(scope, context);
    let global = context.global(scope);

    let data = WorkerContextData {
        outbox,
        onmessage: RwLock::new(None),
    };
    let data_ptr = Box::into_raw(Box::new(data));
    set_worker_data(scope, data_ptr);

    let post_message = v8::Function::new(scope, worker_post_message)
        .ok_or("Failed to create postMessage function")?;
    set_property(scope, global, "postMessage", post_message.into());

    let onmessage_key =
        v8::String::new(scope, "onmessage").ok_or("Failed to create onmessage key")?;
    let _ = global.set_accessor_with_setter(
        scope,
        onmessage_key.into(),
        worker_onmessage_getter,
        worker_onmessage_setter,
    );

    set_property(scope, global, "self", global.into());
    set_property(scope, global, "globalThis", global.into());

    Ok(context)
}

fn execute_script(scope: &mut v8::HandleScope, script: &str) -> Result<(), String> {
    let code = v8::String::new(scope, script).ok_or("Failed to create script string")?;
    let script = v8::Script::compile(scope, code, None).ok_or("Failed to compile worker script")?;
    script
        .run(scope)
        .ok_or_else(|| "Worker script execution failed".to_string())?;
    Ok(())
}

fn dispatch_onmessage(scope: &mut v8::HandleScope, message: WorkerMessage) -> Result<(), String> {
    let handler = {
        let data = get_worker_data(scope);
        data.onmessage.read().clone()
    };

    let Some(handler) = handler else {
        return Ok(());
    };

    let event = create_message_event(scope, message)?;
    let callback = v8::Local::<v8::Function>::new(scope, handler);
    let recv = scope.get_current_context().global(scope).into();
    callback.call(scope, recv, &[event.into()]);
    Ok(())
}

fn create_message_event<'s>(
    scope: &mut v8::HandleScope<'s>,
    message: WorkerMessage,
) -> Result<v8::Local<'s, v8::Object>, String> {
    let event = v8::Object::new(scope);
    let data = match message {
        WorkerMessage::Text(text) => {
            v8::String::new(scope, &text)
                .map(|value| value.into())
                .ok_or("Failed to create message data string")?
        }
        WorkerMessage::Binary(bytes) => {
            let backing_store =
                v8::ArrayBuffer::new_backing_store_from_boxed_slice(bytes.into_boxed_slice());
            let shared_backing_store = backing_store.make_shared();
            v8::ArrayBuffer::with_backing_store(scope, &shared_backing_store).into()
        }
    };
    set_property(scope, event, "data", data);
    Ok(event)
}

fn worker_post_message(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    let data = args.get(0);
    let message = js_value_to_worker_message(scope, data);
    let worker_data = get_worker_data(scope);
    let _ = worker_data.outbox.send(message);
}

fn worker_onmessage_getter(
    scope: &mut v8::HandleScope,
    _name: v8::Local<v8::Name>,
    _args: v8::PropertyCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let data = get_worker_data(scope);
    if let Some(handler) = data.onmessage.read().as_ref() {
        let callback = v8::Local::<v8::Function>::new(scope, handler);
        rv.set(callback.into());
    } else {
        rv.set(v8::null(scope).into());
    }
}

fn worker_onmessage_setter(
    scope: &mut v8::HandleScope,
    _name: v8::Local<v8::Name>,
    value: v8::Local<v8::Value>,
    _args: v8::PropertyCallbackArguments,
) {
    let data = get_worker_data(scope);
    if value.is_null() || value.is_undefined() {
        *data.onmessage.write() = None;
        return;
    }
    if let Ok(callback) = v8::Local::<v8::Function>::try_from(value) {
        *data.onmessage.write() = Some(v8::Global::new(scope, callback));
    }
}

fn js_value_to_worker_message(scope: &mut v8::HandleScope, value: v8::Local<v8::Value>) -> WorkerMessage {
    if value.is_string() {
        let text = value
            .to_string(scope)
            .map(|s| s.to_rust_string_lossy(scope))
            .unwrap_or_default();
        WorkerMessage::Text(text)
    } else if value.is_array_buffer() {
        if let Ok(array_buffer) = v8::Local::<v8::ArrayBuffer>::try_from(value) {
            let backing_store = array_buffer.get_backing_store();
            return WorkerMessage::Binary(backing_store.iter().map(|byte| byte.get()).collect());
        }
        WorkerMessage::Text(String::new())
    } else {
        let text = value
            .to_string(scope)
            .map(|s| s.to_rust_string_lossy(scope))
            .unwrap_or_default();
        WorkerMessage::Text(text)
    }
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

fn set_worker_data<'s>(scope: &mut v8::HandleScope<'s>, data_ptr: *mut WorkerContextData) {
    let global = scope.get_current_context().global(scope);
    let key = v8::String::new(scope, WORKER_DATA_KEY).expect("static V8 key should allocate");
    let external = v8::External::new(scope, data_ptr.cast::<c_void>());
    let _ = global.set(scope, key.into(), external.into());
}

fn worker_data_ptr(scope: &mut v8::HandleScope) -> Option<std::ptr::NonNull<WorkerContextData>> {
    let global = scope.get_current_context().global(scope);
    let key = v8::String::new(scope, WORKER_DATA_KEY)?;
    let value = global.get(scope, key.into())?;
    let external = v8::Local::<v8::External>::try_from(value).ok()?;
    std::ptr::NonNull::new(external.value().cast::<WorkerContextData>())
}

fn get_worker_data(scope: &mut v8::HandleScope) -> &'static WorkerContextData {
    let ptr = worker_data_ptr(scope).expect("worker context data should be installed");
    // SAFETY: `ptr` is owned by the current worker context and freed by `take_worker_data`.
    unsafe { ptr.as_ref() }
}

fn take_worker_data(scope: &mut v8::HandleScope) -> Option<Box<WorkerContextData>> {
    let ptr = worker_data_ptr(scope)?;
    let global = scope.get_current_context().global(scope);
    let key = v8::String::new(scope, WORKER_DATA_KEY)?;
    let undefined = v8::undefined(scope);
    let _ = global.set(scope, key.into(), undefined.into());

    // SAFETY: `ptr` was created with `Box::into_raw` in `setup_worker_context`.
    Some(unsafe { Box::from_raw(ptr.as_ptr()) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn wait_for_messages(worker: &V8Worker) -> Vec<WorkerMessage> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let messages = worker.poll_messages();
            if !messages.is_empty() {
                return messages;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for worker messages");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn test_worker_spawn_and_id() {
        let worker = V8Worker::spawn(1, "var ready = true;").unwrap();
        assert_eq!(worker.id(), 1);
    }

    #[test]
    fn test_worker_post_message_roundtrip() {
        let mut worker = V8Worker::spawn(
            2,
            "self.onmessage = function(event) { self.postMessage('echo:' + event.data); };",
        )
        .unwrap();

        worker
            .post_message(WorkerMessage::Text("hello".to_string()))
            .unwrap();

        let messages = wait_for_messages(&worker);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0], WorkerMessage::Text("echo:hello".to_string()));

        worker.terminate();
    }

    #[test]
    fn test_worker_binary_message() {
        let mut worker = V8Worker::spawn(
            3,
            "self.onmessage = function(event) {
                var bytes = new Uint8Array(event.data);
                self.postMessage('len:' + bytes.length);
             };",
        )
        .unwrap();

        worker
            .post_message(WorkerMessage::Binary(vec![1, 2, 3, 4]))
            .unwrap();

        let messages = wait_for_messages(&worker);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0], WorkerMessage::Text("len:4".to_string()));

        worker.terminate();
    }

    #[test]
    fn test_worker_global_prelude() {
        let mut worker = V8Worker::spawn(
            4,
            "self.postMessage(
                typeof self.postMessage + ':' +
                typeof self.onmessage + ':' +
                (self.globalThis === self)
             );",
        )
        .unwrap();

        let messages = wait_for_messages(&worker);
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0],
            WorkerMessage::Text("function:object:true".to_string())
        );

        worker.terminate();
    }

    #[test]
    fn test_worker_terminate() {
        let mut worker = V8Worker::spawn(5, "self.postMessage('ready');").unwrap();
        let _ = wait_for_messages(&worker);
        worker.terminate();
        assert!(worker.post_message(WorkerMessage::Text("late".to_string())).is_err());
    }

    #[test]
    fn test_worker_message_enum() {
        assert_eq!(
            WorkerMessage::Text("a".to_string()),
            WorkerMessage::Text("a".to_string())
        );
        assert_eq!(
            WorkerMessage::Binary(vec![1, 2]),
            WorkerMessage::Binary(vec![1, 2])
        );
    }
}
