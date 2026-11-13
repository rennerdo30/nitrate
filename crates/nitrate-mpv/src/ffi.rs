//! The libmpv C declarations Nitrate uses (client.h, render.h,
//! render_d3d11.h, client API 2.x). Values are checked against the headers of
//! the mpv build in `docs/BUILD.md`.

use std::ffi::{c_char, c_double, c_int, c_void};

pub type Handle = c_void;
pub type RenderContext = c_void;

pub const FORMAT_NONE: c_int = 0;
pub const FORMAT_STRING: c_int = 1;
pub const FORMAT_FLAG: c_int = 3;
pub const FORMAT_INT64: c_int = 4;
pub const FORMAT_DOUBLE: c_int = 5;

pub const EVENT_NONE: c_int = 0;
pub const EVENT_SHUTDOWN: c_int = 1;
pub const EVENT_LOG_MESSAGE: c_int = 2;
pub const EVENT_START_FILE: c_int = 6;
pub const EVENT_END_FILE: c_int = 7;
pub const EVENT_FILE_LOADED: c_int = 8;
pub const EVENT_VIDEO_RECONFIG: c_int = 17;
pub const EVENT_SEEK: c_int = 20;
pub const EVENT_PLAYBACK_RESTART: c_int = 21;
pub const EVENT_PROPERTY_CHANGE: c_int = 22;

pub const RENDER_PARAM_INVALID: c_int = 0;
pub const RENDER_PARAM_API_TYPE: c_int = 1;
pub const RENDER_PARAM_D3D11_INIT_PARAMS: c_int = 21;
pub const RENDER_PARAM_D3D11_FBO: c_int = 22;
pub const RENDER_UPDATE_FRAME: u64 = 1;

#[repr(C)]
pub struct Event {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct EventProperty {
    pub name: *const c_char,
    pub format: c_int,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct EventEndFile {
    pub reason: c_int,
    pub error: c_int,
    pub playlist_entry_id: i64,
    pub playlist_insert_id: i64,
    pub playlist_insert_num_entries: c_int,
}

#[repr(C)]
pub struct EventLogMessage {
    pub prefix: *const c_char,
    pub level: *const c_char,
    pub text: *const c_char,
    pub log_level: c_int,
}

#[repr(C)]
pub struct RenderParam {
    pub kind: c_int,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct D3d11InitParams {
    pub device: *mut c_void,
}

#[repr(C)]
pub struct D3d11Fbo {
    pub tex: *mut c_void,
}

pub type WakeupFn = unsafe extern "C" fn(*mut c_void);

/// Function pointers resolved from the loaded library. Valid while the
/// `libloading::Library` they came from is alive (`Libmpv` keeps both).
#[derive(Clone, Copy)]
pub struct Api {
    pub error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    pub create: unsafe extern "C" fn() -> *mut Handle,
    pub initialize: unsafe extern "C" fn(*mut Handle) -> c_int,
    pub terminate_destroy: unsafe extern "C" fn(*mut Handle),
    pub set_option_string: unsafe extern "C" fn(*mut Handle, *const c_char, *const c_char) -> c_int,
    pub command_async: unsafe extern "C" fn(*mut Handle, u64, *mut *const c_char) -> c_int,
    pub get_property: unsafe extern "C" fn(*mut Handle, *const c_char, c_int, *mut c_void) -> c_int,
    pub observe_property: unsafe extern "C" fn(*mut Handle, u64, *const c_char, c_int) -> c_int,
    pub request_log_messages: unsafe extern "C" fn(*mut Handle, *const c_char) -> c_int,
    pub wait_event: unsafe extern "C" fn(*mut Handle, c_double) -> *mut Event,
    pub set_wakeup_callback: unsafe extern "C" fn(*mut Handle, Option<WakeupFn>, *mut c_void),
    pub render_context_create: unsafe extern "C" fn(*mut *mut RenderContext, *mut Handle, *mut RenderParam) -> c_int,
    pub render_context_set_update_callback: unsafe extern "C" fn(*mut RenderContext, Option<WakeupFn>, *mut c_void),
    pub render_context_update: unsafe extern "C" fn(*mut RenderContext) -> u64,
    pub render_context_render: unsafe extern "C" fn(*mut RenderContext, *mut RenderParam) -> c_int,
    pub render_context_report_swap: unsafe extern "C" fn(*mut RenderContext),
    pub render_context_free: unsafe extern "C" fn(*mut RenderContext),
}

impl Api {
    /// Resolves every symbol, or names the first one that is missing.
    ///
    /// # Safety
    /// `library` must be a libmpv with client API 2.x, whose functions have
    /// the signatures declared above.
    pub unsafe fn load(library: &libloading::Library) -> Result<Self, (&'static str, String)> {
        macro_rules! sym {
            ($name:literal) => {{
                // SAFETY: the caller guarantees the library is libmpv 2.x; the
                // field type matches the declaration in its headers.
                match unsafe { library.get(concat!($name, "\0").as_bytes()) } {
                    Ok(s) => *s,
                    Err(e) => return Err(($name, e.to_string())),
                }
            }};
        }
        Ok(Self {
            error_string: sym!("mpv_error_string"),
            create: sym!("mpv_create"),
            initialize: sym!("mpv_initialize"),
            terminate_destroy: sym!("mpv_terminate_destroy"),
            set_option_string: sym!("mpv_set_option_string"),
            command_async: sym!("mpv_command_async"),
            get_property: sym!("mpv_get_property"),
            observe_property: sym!("mpv_observe_property"),
            request_log_messages: sym!("mpv_request_log_messages"),
            wait_event: sym!("mpv_wait_event"),
            set_wakeup_callback: sym!("mpv_set_wakeup_callback"),
            render_context_create: sym!("mpv_render_context_create"),
            render_context_set_update_callback: sym!("mpv_render_context_set_update_callback"),
            render_context_update: sym!("mpv_render_context_update"),
            render_context_render: sym!("mpv_render_context_render"),
            render_context_report_swap: sym!("mpv_render_context_report_swap"),
            render_context_free: sym!("mpv_render_context_free"),
        })
    }
}
