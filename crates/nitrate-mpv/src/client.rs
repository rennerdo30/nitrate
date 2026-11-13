//! A libmpv client handle: options, async commands, observed properties and
//! events. Every call here returns quickly; nothing waits for the player core
//! except `get_*` and `wait_event` with a timeout above zero (AGENTS.md rule 1:
//! the UI thread only uses `command`, `observe` and `wait_event(0.0)`).

use crate::Libmpv;
use crate::ffi;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::fmt;
use std::sync::{Arc, Mutex};

/// An error code from libmpv, with its message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: i32,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for Error {}

/// The format an observed property is delivered in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Flag,
    Double,
    Int64,
    String,
}

impl Format {
    fn raw(self) -> c_int {
        match self {
            Format::Flag => ffi::FORMAT_FLAG,
            Format::Double => ffi::FORMAT_DOUBLE,
            Format::Int64 => ffi::FORMAT_INT64,
            Format::String => ffi::FORMAT_STRING,
        }
    }
}

/// A property value. `None` means the property is currently unavailable
/// (for example `duration` with no file loaded).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    None,
    Flag(bool),
    Double(f64),
    Int64(i64),
    String(String),
}

/// Why playback of a file ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    Eof,
    Stop,
    Quit,
    Error,
    Redirect,
    Other(i32),
}

/// An mpv event, copied out of libmpv's buffer.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Shutdown,
    StartFile,
    FileLoaded,
    EndFile { reason: EndReason, error: Option<Error> },
    VideoReconfig,
    Seek,
    PlaybackRestart,
    PropertyChange { id: u64, name: String, value: Value },
    Log { prefix: String, level: String, text: String },
    Other(i32),
}

type Callback = Box<dyn Fn() + Send + Sync>;

pub(crate) struct Inner {
    pub(crate) lib: Arc<Libmpv>,
    pub(crate) handle: *mut ffi::Handle,
    wakeup: Mutex<Option<Box<Callback>>>,
}

// SAFETY: libmpv's client API is thread-safe for one handle (client.h,
// "Thread safety"); the raw pointer is only passed to it.
unsafe impl Send for Inner {}
// SAFETY: as above.
unsafe impl Sync for Inner {}

impl Drop for Inner {
    fn drop(&mut self) {
        // SAFETY: the handle is valid until here; the render context (which
        // holds an `Arc<Inner>`) is already gone, as the docs require.
        unsafe {
            (self.lib.api.set_wakeup_callback)(self.handle, None, std::ptr::null_mut());
            (self.lib.api.terminate_destroy)(self.handle);
        }
    }
}

/// A player instance (`mpv_handle`).
#[derive(Clone)]
pub struct Mpv {
    pub(crate) inner: Arc<Inner>,
}

fn cstring(s: &str) -> CString {
    // Interior NULs cannot be passed to C; they are cut off rather than failing.
    CString::new(s.split('\0').next().unwrap_or_default()).unwrap_or_default()
}

pub(crate) fn error(lib: &Libmpv, code: c_int) -> Error {
    // SAFETY: mpv_error_string accepts any code and returns a static string.
    let ptr = unsafe { (lib.api.error_string)(code) };
    let message = if ptr.is_null() {
        "unknown error".to_owned()
    } else {
        // SAFETY: non-null, NUL-terminated, static.
        unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
    };
    Error { code, message }
}

fn check(lib: &Libmpv, code: c_int) -> Result<(), Error> {
    if code < 0 { Err(error(lib, code)) } else { Ok(()) }
}

/// # Safety
/// `ptr` must be null or a valid NUL-terminated string.
unsafe fn string(ptr: *const c_char) -> String {
    if ptr.is_null() { String::new() } else { unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned() }
}

unsafe extern "C" fn wakeup_trampoline(data: *mut c_void) {
    // SAFETY: `data` is the `Box<Callback>` stored in `Inner::wakeup`, which
    // outlives the registration (it is replaced or cleared only after the
    // callback is unregistered).
    let callback = unsafe { &*(data as *const Callback) };
    // A panic must not unwind into C.
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback));
}

impl Mpv {
    /// Creates an uninitialised player. Set options, then call `initialize`.
    pub fn create(lib: Arc<Libmpv>) -> Result<Self, Error> {
        // SAFETY: plain constructor.
        let handle = unsafe { (lib.api.create)() };
        if handle.is_null() {
            return Err(Error { code: -1, message: "mpv_create failed".to_owned() });
        }
        Ok(Self { inner: Arc::new(Inner { lib, handle, wakeup: Mutex::new(None) }) })
    }

    fn api(&self) -> &ffi::Api {
        &self.inner.lib.api
    }

    /// Sets an option before `initialize` (or a property-like option after).
    pub fn set_option(&self, name: &str, value: &str) -> Result<(), Error> {
        let (n, v) = (cstring(name), cstring(value));
        // SAFETY: valid handle and NUL-terminated strings.
        check(&self.inner.lib, unsafe { (self.api().set_option_string)(self.inner.handle, n.as_ptr(), v.as_ptr()) })
    }

    pub fn initialize(&self) -> Result<(), Error> {
        // SAFETY: valid handle.
        check(&self.inner.lib, unsafe { (self.api().initialize)(self.inner.handle) })
    }

    /// Runs a command without waiting for it (`mpv_command_async`).
    pub fn command(&self, args: &[&str]) -> Result<(), Error> {
        let owned: Vec<CString> = args.iter().map(|a| cstring(a)).collect();
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        // SAFETY: a NULL-terminated array of valid strings; libmpv copies them.
        check(&self.inner.lib, unsafe { (self.api().command_async)(self.inner.handle, 0, ptrs.as_mut_ptr()) })
    }

    /// Asks for `PropertyChange` events with `id` whenever `name` changes.
    pub fn observe(&self, id: u64, name: &str, format: Format) -> Result<(), Error> {
        let n = cstring(name);
        // SAFETY: valid handle and string.
        check(&self.inner.lib, unsafe {
            (self.api().observe_property)(self.inner.handle, id, n.as_ptr(), format.raw())
        })
    }

    /// Reads a double property. Waits for the core: not for the UI thread.
    pub fn get_double(&self, name: &str) -> Result<f64, Error> {
        let n = cstring(name);
        let mut out = 0f64;
        // SAFETY: FORMAT_DOUBLE writes one double.
        check(&self.inner.lib, unsafe {
            (self.api().get_property)(self.inner.handle, n.as_ptr(), ffi::FORMAT_DOUBLE, (&mut out as *mut f64).cast())
        })?;
        Ok(out)
    }

    /// Reads an integer property. Waits for the core: not for the UI thread.
    pub fn get_i64(&self, name: &str) -> Result<i64, Error> {
        let n = cstring(name);
        let mut out = 0i64;
        // SAFETY: FORMAT_INT64 writes one int64.
        check(&self.inner.lib, unsafe {
            (self.api().get_property)(self.inner.handle, n.as_ptr(), ffi::FORMAT_INT64, (&mut out as *mut i64).cast())
        })?;
        Ok(out)
    }

    /// Enables log message events at `level` ("error", "warn", "info", ...).
    pub fn request_log_messages(&self, level: &str) -> Result<(), Error> {
        let l = cstring(level);
        // SAFETY: valid handle and string.
        check(&self.inner.lib, unsafe { (self.api().request_log_messages)(self.inner.handle, l.as_ptr()) })
    }

    /// Calls `callback` (on an mpv thread) whenever events may be pending.
    /// The callback must only signal another thread; it must not call mpv.
    pub fn set_wakeup_callback(&self, callback: impl Fn() + Send + Sync + 'static) {
        let mut slot = self.inner.wakeup.lock().unwrap_or_else(|p| p.into_inner());
        let boxed: Box<Callback> = Box::new(Box::new(callback));
        let data = (&*boxed as *const Callback).cast_mut().cast::<c_void>();
        // SAFETY: the trampoline matches the C signature; `data` stays valid
        // while it is registered because the box is stored below and only
        // dropped after a later registration replaced it.
        unsafe { (self.api().set_wakeup_callback)(self.inner.handle, Some(wakeup_trampoline), data) };
        *slot = Some(boxed);
    }

    /// Returns the next event, waiting up to `timeout` seconds (0: never wait).
    pub fn wait_event(&self, timeout: f64) -> Option<Event> {
        // SAFETY: valid handle; the returned event stays valid until the next
        // wait_event on this handle, and is copied out before returning.
        let event = unsafe { &*(self.api().wait_event)(self.inner.handle, timeout) };
        let lib = &self.inner.lib;
        Some(match event.event_id {
            ffi::EVENT_NONE => return None,
            ffi::EVENT_SHUTDOWN => Event::Shutdown,
            ffi::EVENT_START_FILE => Event::StartFile,
            ffi::EVENT_FILE_LOADED => Event::FileLoaded,
            ffi::EVENT_VIDEO_RECONFIG => Event::VideoReconfig,
            ffi::EVENT_SEEK => Event::Seek,
            ffi::EVENT_PLAYBACK_RESTART => Event::PlaybackRestart,
            ffi::EVENT_END_FILE if !event.data.is_null() => {
                // SAFETY: END_FILE carries an mpv_event_end_file.
                let end = unsafe { &*(event.data as *const ffi::EventEndFile) };
                let reason = match end.reason {
                    0 => EndReason::Eof,
                    2 => EndReason::Stop,
                    3 => EndReason::Quit,
                    4 => EndReason::Error,
                    5 => EndReason::Redirect,
                    r => EndReason::Other(r),
                };
                let error = (end.error < 0).then(|| error(lib, end.error));
                Event::EndFile { reason, error }
            }
            ffi::EVENT_PROPERTY_CHANGE if !event.data.is_null() => {
                // SAFETY: PROPERTY_CHANGE carries an mpv_event_property whose
                // data matches its format.
                let p = unsafe { &*(event.data as *const ffi::EventProperty) };
                let name = unsafe { string(p.name) };
                let value = if p.data.is_null() {
                    Value::None
                } else {
                    match p.format {
                        ffi::FORMAT_FLAG => Value::Flag(unsafe { *(p.data as *const c_int) } != 0),
                        ffi::FORMAT_DOUBLE => Value::Double(unsafe { *(p.data as *const f64) }),
                        ffi::FORMAT_INT64 => Value::Int64(unsafe { *(p.data as *const i64) }),
                        ffi::FORMAT_STRING => Value::String(unsafe { string(*(p.data as *const *const c_char)) }),
                        _ => Value::None,
                    }
                };
                Event::PropertyChange { id: event.reply_userdata, name, value }
            }
            ffi::EVENT_LOG_MESSAGE if !event.data.is_null() => {
                // SAFETY: LOG_MESSAGE carries an mpv_event_log_message.
                let m = unsafe { &*(event.data as *const ffi::EventLogMessage) };
                unsafe { Event::Log { prefix: string(m.prefix), level: string(m.level), text: string(m.text) } }
            }
            other => Event::Other(other),
        })
    }
}

// FORMAT_NONE is part of the ABI table even though Nitrate never observes
// with it; keep the constant referenced so the table stays complete.
const _: c_int = ffi::FORMAT_NONE;
