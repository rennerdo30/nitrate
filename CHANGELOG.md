# Changelog

## Unreleased

- Repository setup: Cargo workspace, CI on Windows, macOS and Linux, Windows setup and release drafts, logo.
- `nitrate-mpv` finds and loads libmpv at runtime and checks its client API version (`nitrate --mpv-info`).
- **Windows MVP player:** mpv renders through its D3D11 render API into Nitrate's own swap chain, with d3d11va hardware decoding on the same device (WARP when there is no GPU). Controls drawn by Nitrate: timeline with hover time, play/pause, time, mute, volume, fullscreen, title; they hide while playing. Open by command line, drag and drop or Ctrl+O; keyboard shortcuts.
- `--smoke` and `--snapshot` for offscreen checks; `scripts\run.bat` builds, fetches the pinned libmpv once and starts Nitrate.
