# Changelog

## Unreleased

- **Website** at nitrate.renner.dev (`website/`, deployed by hand through the Website Pages workflow) and a README with screenshots, install and usage.
- `--start SECS` starts playback at a position, `--pause` opens a file paused, and `--hover F` points a `--snapshot` at the timeline.
- `scripts/screenshots.ps1` retakes the screenshots from the real player with Nitrate's own demo footage (`scripts/demo-footage.py`).

## 0.1.0-beta.1

The first beta: a fast media player for Windows, built on libmpv. macOS and Linux follow.

- Repository setup: Cargo workspace, CI on Windows, macOS and Linux, Windows setup and release drafts, logo.
- `nitrate-mpv` finds and loads libmpv at runtime and checks its client API version (`nitrate --mpv-info`).
- **Windows MVP player:** mpv renders through its D3D11 render API into Nitrate's own swap chain, with d3d11va hardware decoding on the same device (WARP when there is no GPU). Controls drawn by Nitrate: timeline with hover time, play/pause, time, mute, volume, fullscreen, title; they hide while playing. Open by command line, drag and drop or Ctrl+O; keyboard shortcuts.
- `--smoke` and `--snapshot` for offscreen checks; `scripts\run.bat` builds, fetches the pinned libmpv once and starts Nitrate.
- **Windows setup:** ships libmpv-2.dll with its LGPL notice and licence; registers Nitrate per user for 21 video and audio types ("Open with" and Default apps), with a finish-page shortcut to make it the default player. Uninstall removes only Nitrate's own entries. The exe has the Nitrate icon.
- **Updates:** an installed release build checks GitHub releases once a day in the background; when one is found, a notice appears and Ctrl+Shift+U downloads it, checks it against SHA256SUMS.txt, closes Nitrate and lets the setup install and restart it. Never automatically, never in the middle of playback without asking. `NITRATE_NO_UPDATE` turns checks off, `NITRATE_UPDATE_CHANNEL=stable|beta` picks the channel.
