# Nitrate specification

Status: draft, 2026-11-14. Budgets are targets until a benchmark records them (AGENTS.md rule 10).

## 1. Product

Nitrate is a media player for Windows, macOS and Linux. It uses libmpv for decoding, rendering, subtitles and audio, and puts a fast, modern interface around it.

**Why it exists:** mpv plays almost everything better than anything else, but its interface is minimal by design, and good-looking frontends are either platform-only or slow. Nitrate aims to be both the fastest and the nicest player on every platform, including over remote desktop.

**Principles:**
- Speed first: start, open, seek and every UI response (§3).
- One codebase for all three platforms, with each platform's native graphics path (ADR 0001).
- Remote desktop is a target, not an edge case: Windows RDP with and without a GPU, xrdp and VNC on Linux.
- Modern, quiet design: video first, controls appear when needed and get out of the way.

## 2. Scope

### M0: rendering spike (NT-2)
Prove the hardest part before building features: a window that plays a file through libmpv with our controls drawn on top, on all three platforms, passing the gates in §4. The spike decides ADR 0001.

### M1: player
- Open by file, drag and drop, command line and file association.
- Play, pause, seek, frame step, speed, volume, mute.
- Timeline with chapter marks and thumbnail previews on hover.
- Audio and subtitle track menus, external subtitle loading, subtitle delay.
- Playlist: folder, next and previous, shuffle and repeat.
- Resume where you stopped.
- Fullscreen, always-on-top, picture-in-picture-style small window.
- Keyboard shortcuts for everything, rebindable.
- Settings window; mpv options pass through for power users.

### M2: platform polish
- Frameless window with our own title bar.
- Windows media keys, taskbar controls and the system media overlay; macOS Now Playing and media keys; Linux MPRIS.
- Installer integration: file types (opt-in), Start menu, uninstall.
- In-app updates (same contract as the Windows setup's `/S /UPDATE`).

### Later, by owner decision
Streaming URLs, a library view, network shares, screenshots and clips, scripting.

## 3. Speed budgets

Machine for local numbers: the owner PC (record CPU, GPU and driver with every number). Remote numbers: a Windows RDP session to the same PC.

| What | Budget |
|---|---|
| Window with controls visible, cold start | ≤ 150 ms |
| First video frame, local 1080p H.264, hardware decoding | ≤ 300 ms after open |
| Pause, volume, seek response on screen | next present |
| Seek to a keyframe, local file | ≤ 50 ms to the new frame |
| Paused, no UI change | 0 presents, 0 % CPU from Nitrate |
| Playing 4K60 HDR10 HEVC, hardware decoding | 0 dropped frames over 10 minutes |
| CPU and GPU use while playing | within 5 % of plain mpv on the same file |
| Idle memory, no file open | ≤ 60 MB |
| RDP without GPU, 1080p30 | plays at full frame rate with the controls static |

## 4. Spike gates (M0)

The spike is done when every line has a recorded number:

1. Windows local: 4K60 HDR10 HEVC, hardware decoding, controls on top, 0 dropped frames; CPU/GPU within 5 % of plain mpv.
2. Windows RDP, GPU policy off: 1080p30 plays at full rate; controls usable.
3. Windows RDP, GPU policy on: same as 1.
4. Linux Wayland and X11 (one GPU each from Intel/AMD/NVIDIA available on CI or the owner's machines): 4K60 hardware decoding, 0 dropped frames.
5. macOS (Apple silicon): 4K60 HEVC with VideoToolbox, 0 dropped frames.
6. Paused idle: 0 presents and 0 % CPU on all three.

## 5. Architecture

- `nitrate` (`crates/nitrate-app`): the executable. Window, event loop, input, UI.
- `nitrate-mpv`: loads libmpv at runtime and wraps the client and render APIs. No build-time mpv dependency.
- More crates come with the spike: the video output path per platform, the UI renderer, settings.

**libmpv packaging:** Windows packages ship `libmpv-2.dll` next to `nitrate.exe` with its licence and source offer; macOS bundles it in the app; Linux uses the system libmpv (distribution packages) or a bundled copy in a portable archive. Which builds we ship is decided with the spike (LGPL by default).
