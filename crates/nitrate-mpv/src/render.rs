//! mpv's render API with the Direct3D 11 backend (`render_d3d11.h`): mpv
//! draws each video frame into a texture the application owns, on the
//! application's own device and swap chain.

use crate::client::{self, Error, Mpv};
use crate::ffi;
use std::ffi::c_void;
use std::sync::Arc;

type Callback = Box<dyn Fn() + Send + Sync>;

/// A render context. Use it from one thread at a time (the render thread).
pub struct RenderContext {
    mpv: Arc<client::Inner>,
    ctx: *mut ffi::RenderContext,
    update: Option<Box<Callback>>,
}

// SAFETY: render.h allows the mpv_render_* functions on any thread as long as
// only one runs at a time; `&mut self` on every call enforces that.
unsafe impl Send for RenderContext {}

unsafe extern "C" fn update_trampoline(data: *mut c_void) {
    // SAFETY: `data` is the boxed callback owned by the RenderContext, which
    // unregisters it before dropping it.
    let callback = unsafe { &*(data as *const Callback) };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback));
}

impl RenderContext {
    /// Creates a D3D11 render context on `device` (an `ID3D11Device*`).
    ///
    /// # Safety
    /// `device` must be a valid `ID3D11Device` created without
    /// `D3D11_CREATE_DEVICE_SINGLETHREADED` (needed for d3d11va decoding).
    pub unsafe fn new_d3d11(mpv: &Mpv, device: *mut c_void) -> Result<Self, Error> {
        let lib = &mpv.inner.lib;
        let mut init = ffi::D3d11InitParams { device };
        let api_type = c"d3d11";
        let mut params = [
            ffi::RenderParam { kind: ffi::RENDER_PARAM_API_TYPE, data: api_type.as_ptr().cast_mut().cast() },
            ffi::RenderParam {
                kind: ffi::RENDER_PARAM_D3D11_INIT_PARAMS,
                data: (&mut init as *mut ffi::D3d11InitParams).cast(),
            },
            ffi::RenderParam { kind: ffi::RENDER_PARAM_INVALID, data: std::ptr::null_mut() },
        ];
        let mut ctx = std::ptr::null_mut();
        // SAFETY: params are valid for the call; libmpv references the device.
        let code = unsafe { (lib.api.render_context_create)(&mut ctx, mpv.inner.handle, params.as_mut_ptr()) };
        if code < 0 || ctx.is_null() {
            return Err(client::error(lib, code));
        }
        Ok(Self { mpv: mpv.inner.clone(), ctx, update: None })
    }

    /// Calls `callback` (on an mpv thread) when a new frame may be ready or a
    /// redraw is needed. It must only signal the render thread.
    pub fn set_update_callback(&mut self, callback: impl Fn() + Send + Sync + 'static) {
        let boxed: Box<Callback> = Box::new(Box::new(callback));
        let data = (&*boxed as *const Callback).cast_mut().cast::<c_void>();
        // SAFETY: valid context; the box is stored below and outlives the
        // registration.
        unsafe { (self.mpv.lib.api.render_context_set_update_callback)(self.ctx, Some(update_trampoline), data) };
        self.update = Some(boxed);
    }

    /// Returns the update flags; `frame_ready()` tells whether to render.
    pub fn update(&mut self) -> u64 {
        // SAFETY: valid context.
        unsafe { (self.mpv.lib.api.render_context_update)(self.ctx) }
    }

    pub fn frame_ready(flags: u64) -> bool {
        flags & ffi::RENDER_UPDATE_FRAME != 0
    }

    /// Renders the current video frame into `texture` (an `ID3D11Texture2D*`
    /// with `D3D11_BIND_RENDER_TARGET`, on the context's device).
    ///
    /// # Safety
    /// `texture` must be a valid texture as described above. mpv changes the
    /// immediate context's state; the caller sets its own state afterwards.
    pub unsafe fn render_d3d11(&mut self, texture: *mut c_void) -> Result<(), Error> {
        let mut fbo = ffi::D3d11Fbo { tex: texture };
        let mut params = [
            ffi::RenderParam { kind: ffi::RENDER_PARAM_D3D11_FBO, data: (&mut fbo as *mut ffi::D3d11Fbo).cast() },
            ffi::RenderParam { kind: ffi::RENDER_PARAM_INVALID, data: std::ptr::null_mut() },
        ];
        // SAFETY: valid context and params.
        let code = unsafe { (self.mpv.lib.api.render_context_render)(self.ctx, params.as_mut_ptr()) };
        if code < 0 { Err(client::error(&self.mpv.lib, code)) } else { Ok(()) }
    }

    /// Tells mpv a frame was presented (for its frame timing).
    pub fn report_swap(&mut self) {
        // SAFETY: valid context.
        unsafe { (self.mpv.lib.api.render_context_report_swap)(self.ctx) }
    }
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        // SAFETY: unregister the callback before freeing its box, then free the
        // context while the mpv handle (kept alive by `self.mpv`) still exists.
        unsafe {
            (self.mpv.lib.api.render_context_set_update_callback)(self.ctx, None, std::ptr::null_mut());
            (self.mpv.lib.api.render_context_free)(self.ctx);
        }
        self.update = None;
    }
}
