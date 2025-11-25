use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uchar};
use std::sync::{Arc, Mutex, OnceLock};

use tokio::runtime::Runtime;

use crate::client::{Client, ClientCallbacks};

/// C-side callbacks that we expose to C#/C/etc.
struct FfiCallbacks {
    on_joined: Option<extern "C" fn(*const *const c_char, c_int)>,
    on_talk: Option<extern "C" fn(*const c_uchar, c_int)>,
    on_error: Option<extern "C" fn(*const c_char)>,
}

impl Default for FfiCallbacks {
    fn default() -> Self {
        Self {
            on_joined: None,
            on_talk: None,
            on_error: None,
        }
    }
}

// Global singletons – safe in Rust 2024 using OnceLock (no static mut UB)
static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static CLIENT: OnceLock<Arc<Client>> = OnceLock::new();
static CALLBACKS: OnceLock<Mutex<FfiCallbacks>> = OnceLock::new();

fn callbacks() -> &'static Mutex<FfiCallbacks> {
    CALLBACKS.get_or_init(|| Mutex::new(FfiCallbacks::default()))
}

/// Initialize runtime + client.
/// Must be called once before other functions.
/// Returns `true` on success.
#[unsafe(no_mangle)]
pub extern "C" fn pigeonvc_init(server_addr: *const c_char) -> bool {
    // If already initialized, just say OK.
    if CLIENT.get().is_some() {
        return true;
    }

    let addr = unsafe { CStr::from_ptr(server_addr) }
        .to_string_lossy()
        .to_string();

    let rt = match Runtime::new() {
        Ok(r) => r,
        Err(_) => return false,
    };

    let client_res = rt.block_on(async { Client::new(addr).await });
    let client = match client_res {
        Ok(c) => Arc::new(c),
        Err(_) => return false,
    };

    // Store runtime and client; ignore errors if another thread raced us.
    let _ = RUNTIME.set(rt);
    let _ = CLIENT.set(client);

    true
}

/// Validate server using protocol::new_ping / parse_from_server_packet.
/// Returns true if server responded with PONG.
#[unsafe(no_mangle)]
pub extern "C" fn pigeonvc_validate_server() -> bool {
    let client = match CLIENT.get() {
        Some(c) => c.clone(),
        None => return false,
    };

    let runtime = match RUNTIME.get() {
        Some(r) => r,
        None => return false,
    };

    match runtime.block_on(async { client.validate_server().await }) {
        Ok(true) => true,
        _ => false,
    }
}

/// Set C-side callbacks. Safe to call after `pigeonvc_init`.
#[unsafe(no_mangle)]
pub extern "C" fn pigeonvc_set_callbacks(
    joined: Option<extern "C" fn(*const *const c_char, c_int)>,
    talk: Option<extern "C" fn(*const c_uchar, c_int)>,
    error: Option<extern "C" fn(*const c_char)>,
) {
    {
        // Store raw C callbacks
        let mut cb = callbacks().lock().unwrap();
        cb.on_joined = joined;
        cb.on_talk = talk;
        cb.on_error = error;
    }

    let client = match CLIENT.get() {
        Some(c) => c.clone(),
        None => return,
    };

    let runtime = match RUNTIME.get() {
        Some(r) => r,
        None => return,
    };

    // Build Rust-side callbacks that forward into the C callbacks
    let client_callbacks = ClientCallbacks {
        on_joined: Some(|users: Vec<String>| {
            let cb_guard = callbacks().lock().unwrap();
            if let Some(f) = cb_guard.on_joined {
                // Convert Vec<String> -> Vec<CString> -> Vec<*const c_char>
                let cstrings: Vec<CString> = users
                    .iter()
                    .map(|u| CString::new(u.as_str()).unwrap_or_else(|_| CString::new("").unwrap()))
                    .collect();

                let ptrs: Vec<*const c_char> = cstrings.iter().map(|c| c.as_ptr()).collect();

                // Call C callback; user must copy strings synchronously if they want to keep them
                f(ptrs.as_ptr(), ptrs.len() as c_int);
            }
        }),
        on_talk: Some(|audio: Vec<u8>| {
            let cb_guard = callbacks().lock().unwrap();
            if let Some(f) = cb_guard.on_talk {
                f(audio.as_ptr(), audio.len() as c_int);
            }
        }),
        on_error: Some(|err: anyhow::Error| {
            let cb_guard = callbacks().lock().unwrap();
            if let Some(f) = cb_guard.on_error {
                let msg = err.to_string();
                if let Ok(cmsg) = CString::new(msg) {
                    f(cmsg.as_ptr());
                }
            }
        }),
    };

    // Install callbacks into the async Client
    runtime.block_on(async move {
        client.set_callbacks(client_callbacks).await;
    });
}

/// Join with a given username. Returns `true` if the request was queued.
#[unsafe(no_mangle)]
pub extern "C" fn pigeonvc_join(name: *const c_char) -> bool {
    let name = unsafe { CStr::from_ptr(name) }
        .to_string_lossy()
        .to_string();

    let client = match CLIENT.get() {
        Some(c) => c.clone(),
        None => return false,
    };

    let runtime = match RUNTIME.get() {
        Some(r) => r,
        None => return false,
    };

    runtime.spawn(async move {
        let _ = client.join(name).await;
    });

    true
}

/// Send raw PCM audio (already encoded or not – up to you).
#[unsafe(no_mangle)]
pub extern "C" fn pigeonvc_send_audio(buf: *const c_uchar, len: c_int) {
    let client = match CLIENT.get() {
        Some(c) => c.clone(),
        None => return,
    };

    let runtime = match RUNTIME.get() {
        Some(r) => r,
        None => return,
    };

    if buf.is_null() || len <= 0 {
        return;
    }

    let slice = unsafe { std::slice::from_raw_parts(buf, len as usize) };
    let data = slice.to_vec();

    runtime.spawn(async move {
        let _ = client.send_audio(&data).await;
    });
}

/// Start async listener loop (receives Joined/Talked/etc).
#[unsafe(no_mangle)]
pub extern "C" fn pigeonvc_start_listener() {
    let client = match CLIENT.get() {
        Some(c) => c.clone(),
        None => return,
    };

    let runtime = match RUNTIME.get() {
        Some(r) => r,
        None => return,
    };

    runtime.spawn(async move {
        client.start_listener().await;
    });
}

/// Start keepalive loop.
#[unsafe(no_mangle)]
pub extern "C" fn pigeonvc_start_keepalive() {
    let client = match CLIENT.get() {
        Some(c) => c.clone(),
        None => return,
    };

    let runtime = match RUNTIME.get() {
        Some(r) => r,
        None => return,
    };

    runtime.spawn(async move {
        client.start_keepalive().await;
    });
}
