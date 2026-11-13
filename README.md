<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo/nitrate-wordmark-light.svg">
    <img src="assets/logo/nitrate-wordmark.svg" alt="Nitrate" width="420">
  </picture>
</p>

<p align="center"><b>A fast, modern media player for Windows, macOS and Linux.</b><br>
Built in Rust on libmpv. Fast locally, and fast over remote desktop.</p>

---

> **Status: early development (Windows MVP).** Nitrate plays files on Windows with hardware decoding and its own controls. macOS and Linux build but don't play yet. Not ready for everyday use.

## Goals

- **Speed first.** The window appears instantly, seeking answers in the next frame, and a paused player uses no CPU at all.
- **mpv inside.** Decoding, hardware acceleration, HDR, subtitles and audio come from libmpv, so Nitrate plays what mpv plays.
- **Native graphics on every platform.** D3D11 on Windows, and the fastest measured path on macOS and Linux, with hardware-decoded frames going straight to the screen.
- **Remote desktop as a first-class target.** In an RDP, xrdp or VNC session Nitrate switches to a static, animation-free interface, so the remote connection only carries the video.
- **A modern, quiet interface.** Video first; controls appear when you need them.

The full plan, speed budgets and the open rendering decision are in [`docs/SPEC.md`](docs/SPEC.md) and [`docs/decisions/0001-video-and-ui-rendering.md`](docs/decisions/0001-video-and-ui-rendering.md).

## Building

```bat
scripts\run.bat path\to\video.mkv          :: Windows: build, fetch libmpv once, play
scripts\run.bat -Release path\to\video.mkv :: optimized build
```

**Keys:** Space or K play/pause, Left/Right seek 5 s (Shift: 1 s), J/L 10 s, Up/Down volume, M mute, F or double click fullscreen, Esc leave fullscreen, `.` and `,` frame step, Ctrl+O open. Drop a file on the window to play it.

Nitrate loads libmpv at runtime, so building needs no mpv headers. Details: [`docs/BUILD.md`](docs/BUILD.md).

## Licence

Nitrate is released under the [MIT licence](LICENSE). Its Rust dependencies are permissively licensed. libmpv and FFmpeg are separate libraries under their own licences, loaded at runtime and replaceable.
