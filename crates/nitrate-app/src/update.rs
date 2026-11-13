//! In-app updates. The check runs once after the first frame on a worker,
//! at most once a day; installing downloads the setup, verifies it against
//! the release's SHA256SUMS.txt and starts it, then Nitrate closes so the setup
//! can replace it. Nothing here runs on the UI thread.

use nitrate_update::{Candidate, Channel, Version};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::time::SystemTime;

/// Per-user data folder (not the roaming settings folder).
fn data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let d = std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("nitrate"));
    #[cfg(target_os = "macos")]
    let d = std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support/nitrate"));
    #[cfg(not(any(windows, target_os = "macos")))]
    let d = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")))
        .map(|p| p.join("nitrate"));
    d
}

/// Whether this copy was installed by the Nitrate setup (and so can update
/// itself). Portable copies and development builds only report updates.
pub fn installed() -> bool {
    #[cfg(windows)]
    {
        let (Ok(exe), Some(local)) = (std::env::current_exe(), std::env::var_os("LOCALAPPDATA")) else {
            return false;
        };
        let dir = PathBuf::from(local).join("Programs").join("nitrate");
        exe.parent().is_some_and(|p| p.to_string_lossy().eq_ignore_ascii_case(&dir.to_string_lossy()))
    }
    #[cfg(not(windows))]
    false
}

/// Update policy. Until Nitrate has a settings file: `NITRATE_NO_UPDATE`
/// turns checks off and `NITRATE_UPDATE_CHANNEL=stable|beta` picks the channel
/// (default: beta for prerelease builds, stable otherwise).
pub struct Policy {
    pub enabled: bool,
    pub channel: Option<String>,
    /// Check for updates now: ignores the daily limit and says why when
    /// updates are off.
    pub force: bool,
}

pub enum Outcome {
    Found(Candidate),
    UpToDate,
    Skipped,
    /// Updates are off for this copy; the reason, for an explicit check.
    Off(String),
}

/// Runs the daily check if it is due. Called on a worker.
fn check(policy: &Policy) -> Result<Outcome, String> {
    let off = if cfg!(debug_assertions) {
        Some("This is a development build; updates are off. Installed release builds check daily.")
    } else if !policy.enabled || std::env::var_os("NITRATE_NO_UPDATE").is_some() {
        Some("Updates are off (NITRATE_NO_UPDATE is set).")
    } else {
        None
    };
    if let Some(why) = off {
        return Ok(if policy.force { Outcome::Off(why.into()) } else { Outcome::Skipped });
    }
    let current = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|e| e.to_string())?;
    let Some(dir) = data_dir() else { return Ok(Outcome::Skipped) };
    let stamp = dir.join("update-check");
    if !policy.force && !nitrate_update::due(&stamp, SystemTime::now()) {
        return Ok(Outcome::Skipped);
    }
    let channel = Channel::from_setting(policy.channel.as_deref(), &current);
    let result = nitrate_update::check(&current, channel).map_err(|e| e.to_string());
    // A failed check still waits a day: no retry storm on a flaky network.
    let _ = nitrate_update::write_stamp(&stamp, SystemTime::now());
    Ok(match result? {
        Some(c) => Outcome::Found(c),
        None => Outcome::UpToDate,
    })
}

/// Starts the check on a named worker; `done` receives the result. A
/// panic is reported as an error, never taken down with the app.
pub fn spawn_check(policy: Policy, done: impl FnOnce(Result<Outcome, String>) + Send + 'static) {
    let _ = std::thread::Builder::new().name("nitrate-update-check".into()).spawn(move || {
        let r =
            catch_unwind(AssertUnwindSafe(|| check(&policy))).unwrap_or_else(|_| Err("update check crashed".into()));
        done(r);
    });
}

/// Downloads, verifies and starts the setup. Called on a worker. On `Ok`
/// the caller closes Nitrate; the setup waits for it, installs and restarts it.
fn install(c: &Candidate) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = c;
        Err(format!("Download Nitrate {} from {}", c.version, nitrate_update::RELEASE_PAGE))
    }
    #[cfg(windows)]
    {
        if !installed() {
            return Err(format!(
                "This copy was not installed by the setup; get Nitrate {} from {}",
                c.version,
                nitrate_update::RELEASE_PAGE
            ));
        }
        let setup = nitrate_update::download_verified(c).map_err(|e| e.to_string())?;
        // A fresh folder per attempt: never runs a file someone else placed.
        let nonce = SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        let dir = std::env::temp_dir().join(format!("nitrate-update-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&dir).map_err(|e| format!("could not create the download folder: {e}"))?;
        let path = dir.join(nitrate_update::setup_name(&c.version));
        std::fs::write(&path, &setup).map_err(|e| format!("could not save the setup: {e}"))?;
        // Re-check what is on disk right before running it.
        let on_disk = std::fs::read(&path).map_err(|e| e.to_string())?;
        if nitrate_update::sha256_hex(&on_disk) != nitrate_update::sha256_hex(&setup) {
            return Err("the saved setup changed on disk, so it was not run".into());
        }
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        std::process::Command::new(&path)
            .args(["/S", "/UPDATE"])
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()
            .map_err(|e| format!("could not start the setup: {e}"))?;
        Ok(())
    }
}

pub fn spawn_install(c: Candidate, done: impl FnOnce(Result<(), String>) + Send + 'static) {
    let _ = std::thread::Builder::new().name("nitrate-update-install".into()).spawn(move || {
        let r = catch_unwind(AssertUnwindSafe(|| install(&c))).unwrap_or_else(|_| Err("update crashed".into()));
        done(r);
    });
}

/// What the status bar says about an available update.
pub fn notice(version: &Version) -> String {
    if installed() {
        format!("Nitrate {version} is available · Ctrl+Shift+U to install")
    } else {
        format!("Nitrate {version} is available · {}", nitrate_update::RELEASE_PAGE)
    }
}
