# 0001: Video and UI rendering path

Status: **proposed**. The M0 spike (NT-2) decides it with measurements; until then nothing here is final.

## Problem

Nitrate draws its controls on top of mpv's video in one window, must be at least as fast as plain mpv, and must work over remote desktop. Three facts shape the choice:

1. **libmpv's embedding API.** mpv's render API (`libmpv/render.h`) lets an application draw the video into its own surface. As of 2026-11-14 we found only OpenGL and a slow software renderer there; Vulkan and D3D11 are used by standalone mpv but not offered to embedders. The spike re-checks the current headers and changelog before deciding.
2. **Remote desktop.** In Windows RDP sessions without the GPU policy, OpenGL is often a software fallback and Vulkan is usually missing; D3D11 keeps working (on the GPU or on WARP). The owner uses remote desktop, so this decides the Windows path.
3. **Window embedding.** The alternative to the render API, giving mpv a child window (`--wid`), lets mpv use D3D11 or Vulkan itself, but then our controls cannot simply draw over the video (separate windows), and `--wid` does not exist on Wayland.

## Options per platform

| Platform | A: render API into our surface | B: mpv draws itself |
|---|---|---|
| Windows | OpenGL through a D3D11 interop; weak over RDP | `--wid` child with mpv's D3D11 output; controls in a composited layer on top (DirectComposition or a layered window). **Front-runner.** |
| Linux | OpenGL via EGL, works on Wayland and X11. **Front-runner** (Vulkan if the render API offers it) | `--wid` only on X11 |
| macOS | OpenGL (deprecated, still fast; what IINA uses). **Front-runner** | no clean embedding |

The app keeps one interface for "the video surface" so each platform can use its own path while the UI, input and state code stay shared.

## UI toolkit

Rust dependencies must be permissively licensed (AGENTS.md). That rules out toolkits licensed under GPL or a royalty-free/commercial split, however good they are. Candidates the spike compares:
- `winit` for windows and input with our own GPU-drawn overlay (for example `wgpu`, MIT/Apache) on the same surface as the video;
- an own damage-tracked renderer, as ffwd and Kwil use for their remote-desktop-first UIs (`E:/Development/kwil/docs/decisions/0001-renderer.md`).

## Decision criteria

The spike gates in `docs/SPEC.md` §4, in this order: works over RDP, zero dropped frames, CPU/GPU within 5 % of plain mpv, idle at 0 presents, then code size and build time.
