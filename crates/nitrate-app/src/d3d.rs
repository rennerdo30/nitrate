//! The Windows video path: a D3D11 device and a flip-model swap chain on the
//! window, mpv rendering each frame into the back buffer through its D3D11
//! render API, and the UI layers composited on top. All of it runs on one
//! render thread; the UI thread only sends requests (ADR 0001).

use nitrate_mpv::{Mpv, RenderContext};
use nitrate_ui::Layer;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::JoinHandle;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::{Interface, PCSTR, s};

type SnapshotReply = mpsc::Sender<Result<(), String>>;

#[derive(Default)]
struct Requests {
    mpv_update: bool,
    size: Option<(u32, u32)>,
    layers: Option<Arc<Vec<Layer>>>,
    redraw: bool,
    snapshot: Option<(std::path::PathBuf, SnapshotReply)>,
    quit: bool,
}

struct Shared {
    requests: Mutex<Requests>,
    wake: Condvar,
}

impl Shared {
    fn update(&self, f: impl FnOnce(&mut Requests)) {
        let mut r = self.requests.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut r);
        self.wake.notify_one();
    }
}

/// Counters for traces and smoke runs.
#[derive(Default)]
pub struct Stats {
    pub presents: AtomicU64,
    pub video_frames: AtomicU64,
}

/// What the render thread reports once it is up.
pub struct Started {
    pub adapter: String,
    pub software: bool,
}

pub struct Presenter {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    pub stats: Arc<Stats>,
}

impl Presenter {
    /// Starts the render thread for `hwnd` and waits until its device, swap
    /// chain and mpv render context exist (or failed).
    pub fn start(hwnd: isize, size: (u32, u32), mpv: Mpv) -> Result<(Self, Started), String> {
        let shared = Arc::new(Shared { requests: Mutex::new(Requests::default()), wake: Condvar::new() });
        let stats = Arc::new(Stats::default());
        let (tx, rx) = mpsc::channel();
        let thread = {
            let shared = shared.clone();
            let stats = stats.clone();
            std::thread::Builder::new()
                .name("nitrate-render".into())
                .spawn(move || {
                    let hwnd = HWND(hwnd as *mut _);
                    match Renderer::new(hwnd, size, &mpv, shared.clone()) {
                        Ok((renderer, started)) => {
                            let _ = tx.send(Ok(started));
                            renderer.run(&shared, &stats);
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e));
                        }
                    }
                })
                .map_err(|e| e.to_string())?
        };
        let started = rx.recv().map_err(|_| "the render thread stopped during start".to_owned())??;
        shared.update(|r| r.redraw = true);
        Ok((Self { shared, thread: Some(thread), stats }, started))
    }

    pub fn resize(&self, width: u32, height: u32) {
        self.shared.update(|r| r.size = Some((width, height)));
    }

    pub fn set_layers(&self, layers: Arc<Vec<Layer>>) {
        self.shared.update(|r| r.layers = Some(layers));
    }

    /// Draws one frame and writes it (video plus controls) to `path` as PNG.
    /// Blocks until written; for `--snapshot` runs, not for normal use.
    pub fn snapshot(&self, path: std::path::PathBuf) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.shared.update(|r| r.snapshot = Some((path, tx)));
        rx.recv_timeout(std::time::Duration::from_secs(10)).map_err(|_| "snapshot timed out".to_owned())?
    }
}

impl Drop for Presenter {
    fn drop(&mut self) {
        self.shared.update(|r| r.quit = true);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct GpuLayer {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    texture: ID3D11Texture2D,
    view: ID3D11ShaderResourceView,
}

struct Renderer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain1,
    target: Option<(ID3D11Texture2D, ID3D11RenderTargetView)>,
    size: (u32, u32),
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    sampler: ID3D11SamplerState,
    blend: ID3D11BlendState,
    constants: ID3D11Buffer,
    layers: Vec<GpuLayer>,
    // Declared last: dropped after the D3D objects it renders with are done.
    render: RenderContext,
}

const SHADER: &str = r#"
cbuffer Rect : register(b0) { float4 rect; };
struct V { float4 pos : SV_Position; float2 uv : TEXCOORD0; };
V vs(uint id : SV_VertexID) {
    float2 t = float2(id & 1, id >> 1);
    V o;
    o.pos = float4(lerp(rect.x, rect.z, t.x), lerp(rect.y, rect.w, t.y), 0, 1);
    o.uv = t;
    return o;
}
Texture2D tex : register(t0);
SamplerState smp : register(s0);
float4 ps(V i) : SV_Target { return tex.Sample(smp, i.uv); }
"#;

fn compile(entry: PCSTR, target: PCSTR) -> Result<Vec<u8>, String> {
    let mut blob = None;
    let mut errors = None;
    // SAFETY: the source pointer and length describe SHADER; outputs are
    // written to local options.
    let result = unsafe {
        D3DCompile(
            SHADER.as_ptr().cast(),
            SHADER.len(),
            s!("nitrate-overlay"),
            None,
            None,
            entry,
            target,
            0,
            0,
            &mut blob,
            Some(&mut errors),
        )
    };
    if let Err(e) = result {
        let detail = errors
            .map(|b: ID3DBlob| unsafe {
                String::from_utf8_lossy(std::slice::from_raw_parts(
                    b.GetBufferPointer() as *const u8,
                    b.GetBufferSize(),
                ))
                .into_owned()
            })
            .unwrap_or_default();
        return Err(format!("shader compile failed: {e} {detail}"));
    }
    let blob = blob.ok_or("shader compile returned no code")?;
    // SAFETY: the blob owns this many bytes.
    Ok(unsafe { std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize()) }.to_vec())
}

fn create_device() -> Result<(ID3D11Device, ID3D11DeviceContext, bool), String> {
    let levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_10_0];
    // Hardware with video support first (needed for d3d11va decoding), then
    // hardware without it, then WARP (remote sessions without a GPU).
    let attempts = [
        (D3D_DRIVER_TYPE_HARDWARE, D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT, false),
        (D3D_DRIVER_TYPE_HARDWARE, D3D11_CREATE_DEVICE_BGRA_SUPPORT, false),
        (D3D_DRIVER_TYPE_WARP, D3D11_CREATE_DEVICE_BGRA_SUPPORT, true),
    ];
    let mut last = String::new();
    for (driver, flags, software) in attempts {
        let mut device = None;
        let mut context = None;
        // SAFETY: out pointers are valid locals.
        let r = unsafe {
            D3D11CreateDevice(
                None,
                driver,
                HMODULE::default(),
                flags,
                Some(&levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        };
        match (r, device, context) {
            (Ok(()), Some(d), Some(c)) => return Ok((d, c, software)),
            (Err(e), _, _) => last = e.to_string(),
            _ => last = "no device returned".into(),
        }
    }
    Err(format!("no Direct3D 11 device: {last}"))
}

impl Renderer {
    fn new(hwnd: HWND, size: (u32, u32), mpv: &Mpv, shared: Arc<Shared>) -> Result<(Self, Started), String> {
        let (device, context, software) = create_device()?;
        let e = |e: windows::core::Error| e.to_string();
        // SAFETY: COM calls on valid interfaces with valid descriptors.
        unsafe {
            let dxgi: IDXGIDevice = device.cast().map_err(e)?;
            let adapter = dxgi.GetAdapter().map_err(e)?;
            let desc = adapter.GetDesc().map_err(e)?;
            let name = String::from_utf16_lossy(&desc.Description);
            let adapter_name = name.trim_end_matches('\0').to_owned();
            let factory: IDXGIFactory2 = adapter.GetParent().map_err(e)?;
            let sc_desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: size.0.max(1),
                Height: size.1.max(1),
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                Flags: 0,
                Stereo: false.into(),
            };
            let swap_chain = factory.CreateSwapChainForHwnd(&device, hwnd, &sc_desc, None, None).map_err(e)?;
            let _ = factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER);

            let vs_code = compile(s!("vs"), s!("vs_4_0"))?;
            let ps_code = compile(s!("ps"), s!("ps_4_0"))?;
            let mut vs = None;
            device.CreateVertexShader(&vs_code, None, Some(&mut vs)).map_err(e)?;
            let mut ps = None;
            device.CreatePixelShader(&ps_code, None, Some(&mut ps)).map_err(e)?;
            let mut sampler = None;
            let sd = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                MaxLOD: f32::MAX,
                ComparisonFunc: D3D11_COMPARISON_NEVER,
                ..Default::default()
            };
            device.CreateSamplerState(&sd, Some(&mut sampler)).map_err(e)?;
            // Layers are premultiplied: ONE, INV_SRC_ALPHA.
            let mut bd = D3D11_BLEND_DESC::default();
            bd.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: true.into(),
                SrcBlend: D3D11_BLEND_ONE,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            };
            let mut blend = None;
            device.CreateBlendState(&bd, Some(&mut blend)).map_err(e)?;
            let cbd = D3D11_BUFFER_DESC {
                ByteWidth: 16,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                ..Default::default()
            };
            let mut constants = None;
            device.CreateBuffer(&cbd, None, Some(&mut constants)).map_err(e)?;

            let mut render = RenderContext::new_d3d11(mpv, device.as_raw()).map_err(|e| e.to_string())?;
            render.set_update_callback(move || shared.update(|r| r.mpv_update = true));

            let renderer = Self {
                device,
                context,
                swap_chain,
                target: None,
                size,
                vs: vs.ok_or("no vertex shader")?,
                ps: ps.ok_or("no pixel shader")?,
                sampler: sampler.ok_or("no sampler")?,
                blend: blend.ok_or("no blend state")?,
                constants: constants.ok_or("no constant buffer")?,
                layers: Vec::new(),
                render,
            };
            Ok((renderer, Started { adapter: adapter_name, software }))
        }
    }

    fn run(mut self, shared: &Shared, stats: &Stats) {
        loop {
            let req = {
                let mut r = shared.requests.lock().unwrap_or_else(|p| p.into_inner());
                while !(r.mpv_update
                    || r.size.is_some()
                    || r.layers.is_some()
                    || r.redraw
                    || r.snapshot.is_some()
                    || r.quit)
                {
                    r = shared.wake.wait(r).unwrap_or_else(|p| p.into_inner());
                }
                std::mem::take(&mut *r)
            };
            if req.quit {
                return;
            }
            let mut draw = req.redraw;
            if let Some(size) = req.size
                && size != self.size
            {
                self.resize(size);
                draw = true;
            }
            if let Some(layers) = req.layers {
                self.upload(&layers);
                draw = true;
            }
            if req.mpv_update && RenderContext::frame_ready(self.render.update()) {
                stats.video_frames.fetch_add(1, Ordering::Relaxed);
                draw = true;
            }
            if req.snapshot.is_some() {
                draw = true;
            }
            if draw && self.size.0 > 0 && self.size.1 > 0 {
                if let Err(e) = self.frame(req.snapshot) {
                    eprintln!("nitrate: render failed: {e}");
                    return;
                }
                stats.presents.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn resize(&mut self, size: (u32, u32)) {
        self.target = None;
        self.size = size;
        if size.0 == 0 || size.1 == 0 {
            return; // minimised: keep the buffers, draw nothing
        }
        // SAFETY: no outstanding references to the buffers (target dropped).
        let r =
            unsafe { self.swap_chain.ResizeBuffers(0, size.0, size.1, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0)) };
        if let Err(e) = r {
            eprintln!("nitrate: ResizeBuffers failed: {e}");
        }
    }

    fn upload(&mut self, layers: &[Layer]) {
        let old = std::mem::take(&mut self.layers);
        let mut reuse = old.into_iter();
        for l in layers {
            if l.width == 0 || l.height == 0 {
                continue;
            }
            let existing = reuse.next().filter(|g| g.width == l.width && g.height == l.height);
            let gpu = match existing {
                Some(mut g) => {
                    g.x = l.x;
                    g.y = l.y;
                    g
                }
                None => match self.create_layer(l) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("nitrate: overlay texture failed: {e}");
                        continue;
                    }
                },
            };
            // SAFETY: the texture is width x height RGBA8; the data has that size.
            unsafe {
                self.context.UpdateSubresource(&gpu.texture, 0, None, l.rgba.as_ptr().cast(), l.width * 4, 0);
            }
            self.layers.push(gpu);
        }
    }

    fn create_layer(&self, l: &Layer) -> windows::core::Result<GpuLayer> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: l.width,
            Height: l.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            ..Default::default()
        };
        let mut texture = None;
        let mut view = None;
        // SAFETY: valid descriptors and out pointers.
        unsafe {
            self.device.CreateTexture2D(&desc, None, Some(&mut texture))?;
            let texture = texture.ok_or_else(windows::core::Error::empty)?;
            self.device.CreateShaderResourceView(&texture, None, Some(&mut view))?;
            Ok(GpuLayer {
                x: l.x,
                y: l.y,
                width: l.width,
                height: l.height,
                texture,
                view: view.ok_or_else(windows::core::Error::empty)?,
            })
        }
    }

    /// Copies the back buffer to a CPU-readable texture and writes a PNG.
    fn write_snapshot(&self, tex: &ID3D11Texture2D, path: &std::path::Path) -> Result<(), String> {
        let e = |e: windows::core::Error| e.to_string();
        // SAFETY: a staging copy of a texture on this device, mapped for read
        // and unmapped before returning.
        let (w, h, pixels) = unsafe {
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            tex.GetDesc(&mut desc);
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            desc.MiscFlags = 0;
            let mut staging = None;
            self.device.CreateTexture2D(&desc, None, Some(&mut staging)).map_err(e)?;
            let staging = staging.ok_or("no staging texture")?;
            self.context.CopyResource(&staging, tex);
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut m)).map_err(e)?;
            let mut pixels = Vec::with_capacity((desc.Width * desc.Height * 4) as usize);
            for y in 0..desc.Height as usize {
                let row = std::slice::from_raw_parts(
                    (m.pData as *const u8).add(y * m.RowPitch as usize),
                    desc.Width as usize * 4,
                );
                for px in row.as_chunks::<4>().0 {
                    pixels.extend_from_slice(&[px[2], px[1], px[0], 255]); // BGRA -> RGBA
                }
            }
            self.context.Unmap(&staging, 0);
            (desc.Width, desc.Height, pixels)
        };
        let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&pixels).map_err(|e| e.to_string())
    }

    fn frame(&mut self, snapshot: Option<(std::path::PathBuf, SnapshotReply)>) -> Result<(), String> {
        let e = |e: windows::core::Error| e.to_string();
        // SAFETY: COM calls on interfaces owned by this thread.
        unsafe {
            if self.target.is_none() {
                let tex: ID3D11Texture2D = self.swap_chain.GetBuffer(0).map_err(e)?;
                let mut rtv = None;
                self.device.CreateRenderTargetView(&tex, None, Some(&mut rtv)).map_err(e)?;
                self.target = Some((tex, rtv.ok_or("no render target view")?));
            }
            let Some((tex, rtv)) = self.target.clone() else { return Ok(()) };
            self.render.render_d3d11(tex.as_raw()).map_err(|e| e.to_string())?;

            // mpv leaves its own state on the context: set everything we use.
            let ctx = &self.context;
            ctx.OMSetRenderTargets(Some(&[Some(rtv)]), None);
            let (w, h) = (self.size.0 as f32, self.size.1 as f32);
            ctx.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: w,
                Height: h,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            ctx.RSSetState(None);
            ctx.OMSetBlendState(&self.blend, None, 0xffff_ffff);
            ctx.OMSetDepthStencilState(None, 0);
            ctx.IASetInputLayout(None);
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            ctx.VSSetShader(&self.vs, None);
            ctx.PSSetShader(&self.ps, None);
            ctx.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            ctx.VSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            for l in &self.layers {
                let x0 = l.x as f32 / w * 2.0 - 1.0;
                let x1 = (l.x as f32 + l.width as f32) / w * 2.0 - 1.0;
                let y0 = 1.0 - l.y as f32 / h * 2.0;
                let y1 = 1.0 - (l.y as f32 + l.height as f32) / h * 2.0;
                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                ctx.Map(&self.constants, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped)).map_err(e)?;
                std::ptr::copy_nonoverlapping([x0, y0, x1, y1].as_ptr(), mapped.pData.cast::<f32>(), 4);
                ctx.Unmap(&self.constants, 0);
                ctx.PSSetShaderResources(0, Some(&[Some(l.view.clone())]));
                ctx.Draw(4, 0);
            }
            ctx.PSSetShaderResources(0, Some(&[None]));
            if let Some((path, reply)) = snapshot {
                let _ = reply.send(self.write_snapshot(&tex, &path));
            }
            self.swap_chain.Present(1, DXGI_PRESENT(0)).ok().map_err(e)?;
            self.render.report_swap();
        }
        Ok(())
    }
}
