# Nitrate repository rules

Read `E:/Development/AGENTS.md` and `E:/Development/general-agent-rules.md` first. They win over anything here unless this file is stricter.

- **Product:** Nitrate, a media player in Rust for Windows, macOS and Linux, built on libmpv. **P0 is speed**, locally and over remote desktop (owner, 2026-11-14: "really high, high, high performance, even in Windows and remote desktop environments"). It must look modern and feel better than every other media player, not just match them. Scope and budgets: `docs/SPEC.md`.
- **Pair Desk:** project `nitrate` (ids `NT-n`; created 2026-11-14). Handoff, backlog, plans and evidence live there, never in committed files.
- **Identity:** commit only as `rennerdo30 <9086097+rennerdo30@users.noreply.github.com>`.
- **Repository:** public at `https://github.com/rennerdo30/nitrate` (owner, 2026-11-14). Every release needs the owner's yes. Push with explicit refs only: `git push origin HEAD:refs/heads/<branch>`.
- **Public from day one:** no secrets, no owner-PC config in committed files (`.cargo/config.toml` is local), no third-party code, media or assets, and **no other products' names or trademarks** anywhere public: files, code comments, commit messages, PR texts, release notes (owner rule for ffwd, 2026-11-14, applied here too). Exception: the libraries Nitrate ships or loads (libmpv, FFmpeg) are named where licences require it.
- **CI runners:** GitHub-hosted (public repository rule, owner 2026-11-04).
- **Licence:** Nitrate is MIT. Rust dependencies are permissive only (MIT, Apache-2.0, BSD, Zlib, ISC; OFL for fonts), enforced by `deny.toml`. libmpv and FFmpeg are never linked at build time: they are loaded at runtime as separate, replaceable LGPL libraries, shipped with their licence and source offer. Bundling a GPL build of them is an owner decision.
- **Test media:** only files we generate ourselves (FFmpeg test sources) or with a recorded free licence, kept outside Git. Never commit media.
- Paths: always `E:`; `D:` is the same drive. Branch limit: an active branch has a commit in the last 24 h; up to 10 is normal, 11 to 20 needs a recorded reason, 20 is the maximum.

## Performance rules (owner, 2026-11-14)

These hold for every change. A change that breaks one of them is a defect, even if all its tests pass.

1. **Video never waits for the UI, the UI never waits for I/O.** Decoding and rendering belong to mpv's threads and the render path. File probing, thumbnails, playlists, subtitles search, network and settings I/O run on worker threads and report back through the event loop. The UI thread only handles input, layout and painting.
2. **Zero-copy video.** Hardware-decoded frames go to the screen without a CPU readback or copy. A CPU copy of video frames outside the software-decoding fallback is a P0 defect.
3. **Native graphics API per platform, chosen by measurement** (`docs/decisions/0001-video-and-ui-rendering.md`): D3D11 is the Windows path, because it keeps working in remote desktop sessions where OpenGL and Vulkan often fail. No platform path ships without its spike numbers.
4. **Input answers in the next frame.** Seeking, pause, volume and the controls react in the next present with what is already known; the exact frame fills in behind. No spinner for anything under 150 ms.
5. **First frame first.** The window and controls appear before the file is probed; the first video frame shows as soon as mpv delivers it.
6. **Event-driven, damage-only UI.**
   - Paused with no UI change: 0 presents, 0 wakeups from Nitrate, 0 % CPU.
   - Playing: one present per video frame, and the overlay is redrawn only when it changes.
   - No frame loop of our own.
7. **Remote sessions are a first-class target.** Detect them (Windows `SM_REMOTESESSION`, switched live on `WM_WTSSESSION_CHANGE`; xrdp/VNC on Linux; a setting everywhere). In a remote session: no animations, fades, blur, translucency or shadows in the UI; solid fills; static chrome, so the remote codec only sends the video area; a decoder path that works without a GPU.
8. **Never crash on media or the system.**
   - No `unwrap`/`expect` on I/O, media metadata, mpv results, settings or network data.
   - A missing, old or broken libmpv is a message in the window.
   - A panicking worker is caught and reported; the window stays alive.
   - Every crash is a P0 defect with a Pair Desk issue.
9. **Memory is bounded.** Thumbnail, playlist and metadata caches have caps; nothing grows with playback time.
10. **Measured, not assumed.**
    - Every budget in `docs/SPEC.md` gets a benchmark or trace before it is claimed.
    - Dropped frames, present timing and CPU/GPU use are measured against plain mpv on the same file and machine.
    - A regression over 10 % against the recorded number is a defect.
    - Never claim a speed you didn't measure; state the machine, the commit and the file.
11. **Small dependency graph.** Every new dependency earns its place: its pull request states the crate count and the build time it adds. No async runtime on the UI side. Heavy dependencies live in leaf crates.

## Build (fastest iteration; details in `docs/BUILD.md`)

- **Toolchain:** pinned in `rust-toolchain.toml`. Use `rustup ... --no-self-update`; rustup self-updates belong to the owner.
- **Local Cargo config:** `.cargo/config.toml` is machine-local and gitignored; its template is `.cargo/config.example.toml`. A new worktree copies it before its first build. It sets the shared sccache (`SCCACHE_DIR=E:/sccache`), `rust-lld` on Windows, build output in `E:/Development/_cache/cargo-target/nitrate` and `jobs = 8`.
- **Profiles:** in `Cargo.toml`. Dependencies build optimised and cached by sccache; workspace crates stay incremental. Don't set `incremental = false`.
- **Load:** no stress loops, long playback benchmarks or full suites on the owner PC. Use focused tests; full suites run on CI.
- **Agent-launched players:**
  - Always muted (`--mute`), hidden or offscreen, without activation and without a taskbar entry. Never put a window on the owner's screen or play sound on the owner's PC.
  - Every run has a timeout. Record the PID and close only that process.

## Document map

- `docs/SPEC.md`: product scope, speed budgets, architecture, the spike gates. Read it before any work package.
- `docs/decisions/`: architecture decision records. `0001` decides the video and UI rendering path.
- `docs/BUILD.md`: build settings, sccache, linker.
- `docs/RELEASING.md`: releases (drafts only, the owner publishes). Never run `scripts/test-installer.ps1` or a built setup on the owner PC; they run on CI runners only.
- `assets/logo/`: the SVG logo and its derived files.
- `website/`: the homepage at `nitrate.renner.dev` (static, deployed by hand with the Website Pages workflow; `website/README.md`). `docs/images/` and `website/assets/shots/`: real player screenshots from `scripts/screenshots.ps1` (offscreen, muted) with our own footage from `scripts/demo-footage.py`.
- `scripts/`: owner test scripts, batch files on Windows: `run.bat` (build, fetch libmpv, start; `.sh` twin), `check.bat`/`check.sh` (the CI checks), `package.bat` (Windows packages, build only), `make-icon.ps1`, `fetch-libmpv.ps1` (pinned, checksum-verified), `check-crates.sh` (crates.io readiness, no upload; `docs/RELEASING.md`). Agents never run `run.bat` (visible window, sound); they use `nitrate --mute --smoke`/`--snapshot`, which stay offscreen.
