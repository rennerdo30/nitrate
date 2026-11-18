# Releasing Nitrate

Every release needs the owner's yes. CI builds and tests the packages and prepares a **draft** GitHub release. Publishing that draft is the owner's yes; nothing in CI publishes on its own.

## Versions

One version for the whole workspace: `version` under `[workspace.package]` in `Cargo.toml`, following SemVer. Prereleases look like `0.1.0-alpha.1`; their tags contain a `-` and become GitHub prereleases.

## Cutting a release

1. **Prepare the commit on `main`:** set the workspace version, move the `## Unreleased` notes in [`CHANGELOG.md`](../CHANGELOG.md) into a `## X.Y.Z` section, commit, and let CI run.
2. **Tag that commit and push the tag:**
   ```sh
   git tag -a v0.1.0-alpha.1 -m "Nitrate 0.1.0-alpha.1"
   git push origin refs/tags/v0.1.0-alpha.1
   ```
3. **The release workflow runs** ([`.github/workflows/release.yml`](../.github/workflows/release.yml)):
   - checks that the tag matches the workspace version;
   - Windows: builds `nitrate-<version>-setup.exe` (per-user NSIS 3.13 setup) and `nitrate-<version>-windows-x64.zip`, then runs the installer lifecycle test on the disposable runner;
   - macOS (Apple silicon) and Linux (x86_64): builds `nitrate-<version>-<os>-<arch>.tar.gz`;
   - every package carries `LICENSE.txt` and `THIRD-PARTY-NOTICES.txt` (cargo-about, `about.toml`);
   - writes one `SHA256SUMS.txt` over all packages and creates a **draft** release with the changelog section as notes. Rerunning it with the same tag refreshes a draft and never touches a published release.
4. **The owner reviews the draft and publishes it.**

**What the Windows packages contain:** `nitrate.exe` (with its icon), `libmpv-2.dll` (the pinned LGPL build from `scripts/fetch-libmpv.ps1`, checksum-verified), `LICENSE.txt`, `THIRD-PARTY-NOTICES.txt` (Rust crates), `libmpv-NOTICE.txt` (what the DLL is, its licence and where its source is) and `LGPL-2.1.txt`. A new libmpv build means: update the tag, file name and SHA-256 in `fetch-libmpv.ps1` and the commit and links in `licenses/libmpv-NOTICE.txt` together.

**File types:** the setup registers Nitrate per user for the media types in `installer/nitrate.nsi` (`TYPES`): "Open with" entries, a `Nitrate.Media` ProgID and a Default apps entry. It never takes over a default silently; the finish page offers to open Settings > Default apps. Uninstall removes only Nitrate's own values and keeps other apps' entries (the CI test checks this).

## crates.io

Nitrate is also packaged for `cargo install nitrate-player` (the program is still `nitrate`; the name `nitrate` on crates.io belongs to someone else). **Not published yet:** until Nitrate fetches its own libmpv or plays with a system libmpv on macOS and Linux, a Cargo install doesn't give people a working player, so the GitHub setup stays the main install path (Pair Desk NT-5).

- **Four crates, one version, published together:** `nitrate-mpv`, `nitrate-ui`, `nitrate-update`, then `nitrate-player`. The app pins its siblings exactly in `[workspace.dependencies]`; bump those pins with the workspace version.
- **What each crate ships:** its `include` list in its `Cargo.toml`, its own `README.md` (absolute links, so they work on crates.io) and a `LICENSE` copy. The exe icon lives in `crates/nitrate-app/nitrate.ico`, so a crates.io build embeds it too.
- **Checked on every push:** CI job `crates` runs `scripts/check-crates.sh`: pinned versions match, LICENSE copies match the root, `cargo package` builds every crate from its packaged files alone, and each `.crate` stays under 10 MB. Nothing is uploaded.
- **Publishing** needs the owner's yes, after the GitHub release for the same tag is published:
  1. Once: create a crates.io API token with the publish scope only, and add it as the secret `CARGO_REGISTRY_TOKEN` in a repository environment `crates-io` with the owner as required reviewer.
  2. Run the **publish-crate** workflow from the Actions tab with the tag. It checks that the tag matches the workspace version and is on `main`, runs the package check without the token, and only then runs `cargo publish --workspace`.
- crates.io versions can't be deleted, only yanked. A wrong upload means a new version.

## Updates in the app

- An installed release build (in `%LOCALAPPDATA%\Programs\nitrate`) checks GitHub releases once after start, at most once a day, on a worker thread. Development builds, `--smoke`/`--snapshot` runs and `NITRATE_NO_UPDATE` never check.
- A found update shows a notice in the top right. **Ctrl+Shift+U** installs it: Nitrate downloads the setup and `SHA256SUMS.txt` into a fresh temporary folder, checks the SHA-256, starts the setup with `/S /UPDATE` and closes; the setup waits for it, installs and starts Nitrate again. Nothing installs without that key press.
- **Channels:** a prerelease build follows beta (newest published release, prerelease or stable), a stable build follows stable; `NITRATE_UPDATE_CHANNEL=stable|beta` overrides.
- Only GitHub's API for `rennerdo30/nitrate` and its release downloads are fetched, over HTTPS (WinHTTP, so system proxy settings apply), size-capped, 60 s timeouts.
- The checksum protects integrity, not authenticity (it comes from the same release). The setup is not code-signed yet, so SmartScreen warns on first run; signing is a separate owner decision.

The setup's file name `nitrate-<version>-setup.exe` and `SHA256SUMS.txt` are the contract with the updater; keep both names.

## Building the packages locally

`scripts\package.bat` builds the release binary, the setup, the zip and `SHA256SUMS.txt` into `artifacts\installer`. It uses the portable NSIS 3.13 at `E:/Development/_cache/tools/nsis-3.13/`; `scripts/fetch-nsis.ps1` downloads the pinned ZIP elsewhere. Without cargo-about it writes a placeholder notice; release builds always generate the real one in CI.

**Never run `scripts/test-installer.ps1` on a real PC.** It installs and uninstalls for the current user, and refuses to run outside GitHub Actions.
