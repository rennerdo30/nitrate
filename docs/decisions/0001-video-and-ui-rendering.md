# 0001: Video and UI rendering path

Status: **decided for Windows** (2026-11-14, MVP); macOS and Linux still open.

## Problem

Nitrate draws its controls on top of mpv's video in one window, must be at least as fast as plain mpv, and must work over remote desktop.

## Facts

1. **libmpv's render API has a Direct3D 11 backend.** mpv's `render.h` lists D3D11 (`MPV_RENDER_API_TYPE_D3D11`, `render_d3d11.h`, params 21 and 22), OpenGL and software. Checked in the headers and source of mpv `fe753a7824` (2026-11-13). mpv renders a frame into an `ID3D11Texture2D` we own, on our device; d3d11va decodes on the same device.
2. **Remote desktop.** In Windows RDP sessions OpenGL is often a slow software fallback and Vulkan usually missing; D3D11 keeps working, on the GPU or on WARP.
3. **Window embedding (`--wid`)** puts mpv in a child window our controls cannot draw over, and does not exist on Wayland.

## Decision (Windows)

- One window (winit). A render thread owns a D3D11 device (hardware with video support, then hardware, then WARP) and a flip-model swap chain (`FLIP_DISCARD`, 2 buffers, `B8G8R8A8_UNORM`).
- mpv renders each frame straight into the back buffer (`mpv_render_context_render` with `MPV_RENDER_PARAM_D3D11_FBO`). Hardware decoding: `hwdec=auto-safe` (d3d11va on our device, zero-copy).
- The controls are CPU-drawn (tiny-skia, fontdue) into small premultiplied RGBA layers, uploaded only when they change, and composited over the video with one textured quad each.
- The render thread wakes only on: an mpv frame, a layer change, a resize, a snapshot. Paused and idle: no presents.

**Measured** (owner PC, AMD Radeon RX 9070 XT, 1080p30 H.264, release build, `--smoke 8`): d3d11va active, 0 dropped and 0 delayed frames, presents equal to video frames plus layer changes, 3 presents in 3 s idle.

## Open

- macOS (OpenGL via the render API, or Metal later) and Linux (OpenGL via EGL; Vulkan if the render API gets it).
- Spike gates still to record (docs/SPEC.md §4): 4K60 HDR, RDP with and without GPU, CPU/GPU against plain mpv.

## UI toolkit

Rust dependencies must be permissively licensed (AGENTS.md), so toolkits under GPL or a royalty-free/commercial split are out. The controls are drawn by `nitrate-ui` (tiny-skia BSD-3-Clause, fontdue MIT/Apache/Zlib), as ffwd and Kwil draw their own UIs.
