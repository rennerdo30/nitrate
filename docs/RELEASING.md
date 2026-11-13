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

Packages do not contain libmpv yet; bundling it (with its licence and source offer) comes with the spike (`docs/SPEC.md` §5).

## Building the packages locally

`scripts\package.bat` builds the release binary, the setup, the zip and `SHA256SUMS.txt` into `artifacts\installer`. It uses the portable NSIS 3.13 at `E:/Development/_cache/tools/nsis-3.13/`; `scripts/fetch-nsis.ps1` downloads the pinned ZIP elsewhere. Without cargo-about it writes a placeholder notice; release builds always generate the real one in CI.

**Never run `scripts/test-installer.ps1` on a real PC.** It installs and uninstalls for the current user, and refuses to run outside GitHub Actions.
