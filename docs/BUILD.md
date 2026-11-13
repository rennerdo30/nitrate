# Building Nitrate

```sh
cargo build --release
cargo run -- --mpv-info      # which libmpv Nitrate finds
```

The toolchain is pinned in `rust-toolchain.toml` (Rust 1.100.0); `rustup` installs it automatically.

**libmpv** is loaded at runtime, so building needs no mpv headers or libraries. To run playback (from the spike on), put `libmpv-2.dll` next to `nitrate.exe` on Windows, or install libmpv from your package manager (`libmpv2` on Debian and Ubuntu, `mpv-libs` on Fedora, `mpv` from Homebrew on macOS).

## Fastest iteration

| Setting | Where | Why |
|---|---|---|
| `[profile.dev] debug = "line-tables-only"` | `Cargo.toml` | Faster codegen and linking, a smaller target folder |
| `[profile.dev.package."*"] opt-level = 2, debug = false` | `Cargo.toml` | Dependencies run fast in dev and build once (then come from sccache) |
| Incremental stays on for workspace crates | default | sccache passes these through uncached; incremental is the fast path for them |
| `rustc-wrapper` = shared sccache | `.cargo/config.toml` (local) | Dependencies compile once per machine, shared across worktrees and projects |
| `linker = "rust-lld.exe"` (Windows) | `.cargo/config.toml` (local) | Several times faster linking than `link.exe` |
| `target-dir` outside the repository | `.cargo/config.toml` (local) | One build folder per project, outside the source tree |
| `jobs = 8` | `.cargo/config.toml` (local) | Leaves headroom on a shared PC |
| `[profile.release] lto = "thin", codegen-units = 1` | `Cargo.toml` | Fast, small release binaries; unwinding stays on so worker panics are caught |

`.cargo/config.toml` is machine-local and gitignored. Copy `.cargo/config.example.toml` and adjust its paths. Without it, a plain `cargo build` works too.

## CI

GitHub-hosted runners (Windows, macOS, Linux) run `cargo fmt --check`, `clippy -D warnings`, the tests and a release build, with sccache backed by the GitHub Actions cache. A Windows job builds the setup and runs its install, reinstall and uninstall test. `cargo-deny` enforces the permissive licence list in `deny.toml`.

**Windows note:** release builds use the GUI subsystem, so a shell does not wait for them and their output is not shown. From a terminal, use the debug build or `Start-Process -Wait -RedirectStandardOutput`.
