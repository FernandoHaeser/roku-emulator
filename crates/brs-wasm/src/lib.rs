//! WebAssembly entry points for the browser build. Plain C ABI (no
//! wasm-bindgen): the worker copies a channel zip into wasm memory, then calls
//! `brs_run` repeatedly with a time budget and reads console output back.
//!
//! The VM never blocks: `Sleep()` returns a wake-up time and the worker
//! schedules the next slice, so the page stays responsive.
//!
//! Data exchange uses two buffers owned by the module:
//! - the *scratch* buffer (`brs_scratch`) for small JS -> wasm strings
//!   (remote key names, file paths, registry JSON);
//! - the *blob* buffer (`brs_blob_ptr`/`brs_blob_len`) for wasm -> JS payloads
//!   (display list, package files, registry JSON, channel metadata).
//!
//! Both are only valid until the next call into the module.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use brs_core::sg::{render, runtime};
use brs_core::{AssocArray, Channel, Host, Object, Status, Value, Vm};

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "host")]
extern "C" {
    fn debug_wait(output: *const u8, output_len: usize, command: *mut u8, capacity: usize) -> i32;
    fn debug_resumed();
    fn http_request(
        request: *const u8,
        length: usize,
        id: u32,
        response: *mut u8,
        capacity: usize,
    ) -> i32;
    fn http_cancel(id: u32);
}

struct WasmHost {
    output: Rc<RefCell<Vec<u8>>>,
    now_ms: Rc<Cell<f64>>,
    /// `Date.now() - performance.now()` measured by the worker at load time.
    epoch_offset_ms: f64,
}

impl Host for WasmHost {
    fn write(&mut self, text: &str) {
        self.output.borrow_mut().extend_from_slice(text.as_bytes());
    }

    fn now_ms(&self) -> f64 {
        self.now_ms.get()
    }

    fn epoch_ms(&self) -> f64 {
        self.now_ms.get() + self.epoch_offset_ms
    }
    fn debug_wait(&mut self) -> Option<String> {
        #[cfg(target_arch = "wasm32")]
        {
            let mut command = vec![0u8; 65_536];
            let output = self.output.borrow();
            // SAFETY: both buffers remain allocated throughout the host call.
            let size = unsafe {
                debug_wait(
                    output.as_ptr(),
                    output.len(),
                    command.as_mut_ptr(),
                    command.len(),
                )
            };
            drop(output);
            if size < 0 {
                return None;
            }
            self.output.borrow_mut().clear();
            return String::from_utf8(command[..(size as usize).min(command.len())].to_vec()).ok();
        }
        #[cfg(not(target_arch = "wasm32"))]
        None
    }
    fn http(&mut self, request: &str) -> String {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = request;
        #[cfg(target_arch = "wasm32")]
        {
            let mut result = vec![0u8; 12 * 1024 * 1024];
            // SAFETY: buffers remain allocated throughout the synchronous host call.
            let length = unsafe {
                http_request(
                    request.as_ptr(),
                    request.len(),
                    0,
                    result.as_mut_ptr(),
                    result.len(),
                )
            };
            if length >= 0 {
                return String::from_utf8_lossy(&result[..(length as usize).min(result.len())])
                    .into_owned();
            }
        }
        "{\"status\":-7,\"failure\":\"Synchronous HTTP requires an isolated browser host\"}".into()
    }
    fn http_start(&mut self, id: u32, request: &str) -> bool {
        #[cfg(target_arch = "wasm32")]
        // SAFETY: request is readable for the duration of this host call; no output buffer for async.
        unsafe {
            return http_request(request.as_ptr(), request.len(), id, std::ptr::null_mut(), 0) >= 0;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (id, request);
            false
        }
    }
    fn http_cancel(&mut self, id: u32) {
        #[cfg(target_arch = "wasm32")]
        // SAFETY: no pointers cross the host boundary.
        unsafe {
            http_cancel(id);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = id;
    }
    fn debug_resumed(&mut self) {
        #[cfg(target_arch = "wasm32")]
        // SAFETY: no pointers cross this notification.
        unsafe {
            debug_resumed();
        }
    }
}

#[derive(Default)]
struct State {
    vm: Option<Vm>,
    output: Rc<RefCell<Vec<u8>>>,
    now_ms: Rc<Cell<f64>>,
    sleep_until_ms: f64,
    /// Snapshot handed to JS by `brs_output_ptr`/`brs_output_len`.
    pending: Vec<u8>,
    scratch: Vec<u8>,
    blob: Vec<u8>,
    /// Icon bytes of the last inspected package (`brs_take_icon`).
    inspected_icon: Vec<u8>,
    launch_args: AssocArray,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Status codes shared with apps/web/src/worker/engine.ts.
const STATUS_RUNNING: i32 = 0;
const STATUS_SLEEPING: i32 = 1;
const STATUS_FINISHED: i32 = 2;
const STATUS_CRASHED: i32 = 3;
const STATUS_STOPPED: i32 = 4;
const STATUS_WAITING: i32 = 5;
const STATUS_NOT_LOADED: i32 = -1;

/// Return values of the blob producers when there is nothing to hand over.
const BLOB_UNCHANGED: i32 = -1;
const BLOB_NONE: i32 = -2;

/// Allocates `len` bytes for JS to write into. Ownership returns via
/// `brs_load_zip` / `brs_inspect_zip`.
#[no_mangle]
pub extern "C" fn brs_alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len);
    let pointer = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    pointer
}

/// Resizes the scratch buffer to `len` bytes and returns its address.
#[no_mangle]
pub extern "C" fn brs_scratch(len: usize) -> *mut u8 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.scratch.clear();
        state.scratch.resize(len, 0);
        state.scratch.as_mut_ptr()
    })
}

#[no_mangle]
pub extern "C" fn brs_blob_ptr() -> *const u8 {
    STATE.with(|state| state.borrow().blob.as_ptr())
}

#[no_mangle]
pub extern "C" fn brs_blob_len() -> usize {
    STATE.with(|state| state.borrow().blob.len())
}

/// Takes back a zip written by JS into a `brs_alloc` buffer.
///
/// # Safety
/// `ptr` must come from `brs_alloc(len)` and be fully initialized by the caller.
unsafe fn take_buffer(ptr: *mut u8, len: usize) -> Vec<u8> {
    // SAFETY: guaranteed by the caller contract above; we take ownership back.
    unsafe { Vec::from_raw_parts(ptr, len, len) }
}

fn scratch_text(state: &State) -> String {
    String::from_utf8_lossy(&state.scratch).into_owned()
}

/// Loads and launches a channel from a zip previously written at `ptr`.
/// Returns 0 on success; on failure the error text is in the output buffer.
///
/// # Safety
/// `ptr` must come from `brs_alloc(len)` and be fully initialized by the caller.
#[no_mangle]
pub unsafe extern "C" fn brs_load_zip(
    ptr: *mut u8,
    len: usize,
    now_ms: f64,
    epoch_offset_ms: f64,
) -> i32 {
    // SAFETY: forwarded caller contract.
    let zip = unsafe { take_buffer(ptr, len) };
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.now_ms.set(now_ms);
        state.vm = None;
        state.output.borrow_mut().clear();
        let result = Channel::from_zip(&zip)
            .map_err(|e| e.to_string())
            .and_then(|channel| {
                let host = WasmHost {
                    output: state.output.clone(),
                    now_ms: state.now_ms.clone(),
                    epoch_offset_ms,
                };
                brs_core::launch_with_args(&channel, Box::new(host), state.launch_args.clone())
            });
        match result {
            Ok(vm) => {
                state.vm = Some(vm);
                0
            }
            Err(message) => {
                state
                    .output
                    .borrow_mut()
                    .extend_from_slice(format!("{message}\n").as_bytes());
                1
            }
        }
    })
}

/// Drops the running channel (Home key / exit to the home screen). Prints
/// the OS exit lines first, so read the output after calling this.
#[no_mangle]
pub extern "C" fn brs_unload() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if let Some(vm) = state.vm.as_mut() {
            vm.report_exit();
        }
        state.vm = None;
    });
}

/// Reads a package's manifest without launching it, for the home screen.
/// On success returns 0 and the blob holds `{"title","version","icon"}`
/// (icon = whether `brs_take_icon` has bytes); returns 1 with the error text
/// in the blob otherwise.
///
/// # Safety
/// `ptr` must come from `brs_alloc(len)` and be fully initialized by the caller.
#[no_mangle]
pub unsafe extern "C" fn brs_inspect_zip(ptr: *mut u8, len: usize) -> i32 {
    // SAFETY: forwarded caller contract.
    let zip = unsafe { take_buffer(ptr, len) };
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        match Channel::from_zip(&zip) {
            Ok(channel) => {
                let icon = ["mm_icon_focus_fhd", "mm_icon_focus_hd", "mm_icon_focus_sd"]
                    .iter()
                    .filter_map(|key| channel.manifest.get(*key))
                    .find_map(|uri| {
                        let path = uri.trim().strip_prefix("pkg:/")?.trim_start_matches('/');
                        channel.files.get(path).cloned()
                    })
                    .unwrap_or_default();
                let mut info = AssocArray::new();
                info.set("title", Value::string(channel.title().as_str()));
                info.set("version", Value::string(channel.version().as_str()));
                info.set("icon", Value::Boolean(!icon.is_empty()));
                let json = brs_core::json::format(&Value::object(Object::AssocArray(info)))
                    .unwrap_or_default();
                state.blob = json.into_bytes();
                state.inspected_icon = icon;
                0
            }
            Err(error) => {
                state.blob = error.to_string().into_bytes();
                state.inspected_icon.clear();
                1
            }
        }
    })
}

/// Moves the last inspected icon into the blob; returns its length.
#[no_mangle]
pub extern "C" fn brs_take_icon() -> usize {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.blob = std::mem::take(&mut state.inspected_icon);
        state.blob.len()
    })
}

/// Executes up to `budget` instructions.
#[no_mangle]
pub extern "C" fn brs_run(budget: u32, now_ms: f64) -> i32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.now_ms.set(now_ms);
        let Some(vm) = state.vm.as_mut() else {
            return STATUS_NOT_LOADED;
        };
        match vm.run(u64::from(budget)) {
            Status::Running => STATUS_RUNNING,
            Status::Sleeping { until_ms } => {
                state.sleep_until_ms = until_ms;
                STATUS_SLEEPING
            }
            Status::Finished => STATUS_FINISHED,
            Status::Crashed => STATUS_CRASHED,
            Status::Stopped => STATUS_STOPPED,
            Status::Waiting => STATUS_WAITING,
        }
    })
}

#[no_mangle]
pub extern "C" fn brs_sleep_until() -> f64 {
    STATE.with(|state| state.borrow().sleep_until_ms)
}

/// Remote key (name in the scratch buffer, ECP spelling such as "OK").
#[no_mangle]
pub extern "C" fn brs_key(pressed: i32) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let key = scratch_text(&state);
        if let Some(vm) = state.vm.as_mut() {
            runtime::queue_key(vm, &key, pressed != 0);
        }
    })
}

/// SceneGraph revision; bumps whenever something visible may have changed.
#[no_mangle]
pub extern "C" fn brs_revision() -> f64 {
    STATE.with(|state| {
        state
            .borrow()
            .vm
            .as_ref()
            .map_or(0.0, |vm| vm.sg.revision as f64)
    })
}

/// Fills the blob with the display list JSON when the revision differs from
/// `known_revision`. Returns its length, `BLOB_UNCHANGED`, or `BLOB_NONE`
/// when no scene is shown.
#[no_mangle]
pub extern "C" fn brs_display_list(known_revision: f64) -> i32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(vm) = state.vm.as_ref() else {
            return BLOB_NONE;
        };
        if vm.sg.revision as f64 == known_revision {
            return BLOB_UNCHANGED;
        }
        let list = render::display_list(vm);
        match list {
            Some(list) => {
                state.blob = list.into_bytes();
                state.blob.len() as i32
            }
            None => BLOB_NONE,
        }
    })
}

/// Package file by device path (in the scratch buffer, e.g. "pkg:/images/a.png").
/// Returns its length with the bytes in the blob, or `BLOB_NONE`.
#[no_mangle]
pub extern "C" fn brs_package_file() -> i32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let uri = scratch_text(&state);
        let data = state
            .vm
            .as_ref()
            .and_then(|vm| vm.env.package_file(&uri).cloned());
        match data {
            Some(data) => {
                state.blob = data;
                state.blob.len() as i32
            }
            None => BLOB_NONE,
        }
    })
}

/// Hands Video playback to the host player (`1`) or keeps the engine's
/// simulated backend (`0`). Call after `brs_load_zip`.
#[no_mangle]
pub extern "C" fn brs_video_external(external: i32) {
    STATE.with(|state| {
        if let Some(vm) = state.borrow_mut().vm.as_mut() {
            brs_core::sg::video::set_external(vm, external != 0);
        }
    })
}

/// Pending Video player commands as a JSON array in the blob, or `BLOB_NONE`.
#[no_mangle]
pub extern "C" fn brs_video_commands() -> i32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let commands = state
            .vm
            .as_mut()
            .map(brs_core::sg::video::take_commands)
            .unwrap_or_default();
        if commands.is_empty() {
            return BLOB_NONE;
        }
        state.blob = format!("[{}]", commands.join(",")).into_bytes();
        state.blob.len() as i32
    })
}

/// Host player event (JSON in the scratch buffer), see `video::host_event`.
#[no_mangle]
pub extern "C" fn brs_video_event() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let event = scratch_text(&state);
        if let Some(vm) = state.vm.as_mut() {
            brs_core::sg::video::host_event(vm, &event);
        }
    })
}

/// Replaces the channel registry with the JSON in the scratch buffer.
/// Called right after `brs_load_zip`, before the first `brs_run`.
#[no_mangle]
pub extern "C" fn brs_registry_load() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let json = scratch_text(&state);
        if let Some(vm) = state.vm.as_mut() {
            vm.env.load_registry_json(&json);
            vm.env.registry_dirty = false;
        }
    })
}

/// When the registry changed since the last call, puts its JSON in the blob
/// and returns the length; otherwise `BLOB_UNCHANGED`.
#[no_mangle]
pub extern "C" fn brs_registry_take() -> i32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(vm) = state.vm.as_mut() else {
            return BLOB_NONE;
        };
        if !vm.env.registry_dirty {
            return BLOB_UNCHANGED;
        }
        vm.env.registry_dirty = false;
        let json = vm.env.registry_json().into_bytes();
        state.blob = json;
        state.blob.len() as i32
    })
}

/// Moves pending console output into a stable buffer and returns its address.
#[no_mangle]
pub extern "C" fn brs_take_output() -> *const u8 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let output = std::mem::take(&mut *state.output.borrow_mut());
        state.pending = output;
        state.pending.as_ptr()
    })
}

#[no_mangle]
pub extern "C" fn brs_output_len() -> usize {
    STATE.with(|state| state.borrow().pending.len())
}

/// Executes a debugger command from the scratch buffer. Returns 1 if resumed.
#[no_mangle]
pub extern "C" fn brs_debugger_command() -> i32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let command = String::from_utf8_lossy(&state.scratch).into_owned();
        state
            .vm
            .as_mut()
            .map_or(0, |vm| i32::from(vm.debugger_command(&command)))
    })
}

/// Completes an asynchronous HTTP request using the scratch buffer response.
#[no_mangle]
pub extern "C" fn brs_http_complete(id: u32) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let response = String::from_utf8_lossy(&state.scratch).into_owned();
        if let Some(vm) = &mut state.vm {
            brs_core::runtime_objects::complete_http(vm, id, &response);
        }
    });
}

#[no_mangle]
pub extern "C" fn brs_launch_args() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.launch_args = match brs_core::json::parse(&scratch_text(&state)) {
            Ok(Value::Object(o)) => match &*o.borrow() {
                Object::AssocArray(a) => a.clone(),
                _ => AssocArray::new(),
            },
            _ => AssocArray::new(),
        };
    });
}
#[no_mangle]
pub extern "C" fn brs_input() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let info = brs_core::json::parse(&scratch_text(&state)).unwrap_or(Value::Invalid);
        if let Some(vm) = &mut state.vm {
            brs_core::runtime_objects::deliver_input(vm, info);
        }
    });
}

/// Drains remote Poster URIs for the host transport.
#[no_mangle]
pub extern "C" fn brs_image_requests() -> i32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let requests = state
            .vm
            .as_mut()
            .map(brs_core::sg::bitmap::take_requests)
            .unwrap_or_default();
        let value = Value::object(Object::Array(
            requests.into_iter().map(Value::string).collect(),
        ));
        state.blob = brs_core::json::format(&value)
            .unwrap_or_else(|_| "[]".into())
            .into_bytes();
        state.blob.len() as i32
    })
}

/// Supplies the remote Poster header result from scratch JSON.
#[no_mangle]
pub extern "C" fn brs_image_complete() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Ok(Value::Object(result)) = brs_core::json::parse(&scratch_text(&state)) else {
            return;
        };
        let result = result.borrow();
        let Object::AssocArray(result) = &*result else {
            return;
        };
        let Some(uri) = result.get("uri").and_then(Value::as_str) else {
            return;
        };
        let size = result
            .get("width")
            .and_then(Value::as_f64)
            .zip(result.get("height").and_then(Value::as_f64))
            .filter(|(w, h)| *w > 0.0 && *h > 0.0)
            .map(|(w, h)| (w as u32, h as u32));
        if let Some(vm) = state.vm.as_mut() {
            brs_core::sg::bitmap::complete(vm, uri, size);
        }
    });
}
