//! Loads libmpv at runtime.
//!
//! Nitrate links no mpv code at build time. The player finds the shared
//! library when it starts, so:
//! - the workspace builds and tests on any machine without mpv headers;
//! - libmpv (LGPL) stays a separate, replaceable file next to `nitrate`;
//! - a missing or too old libmpv is a message in the window, never a crash
//!   (AGENTS.md performance rule 8).
//!
//! Search order: the folder of the running executable first, then the
//! system's normal library search (`PATH` on Windows, the dynamic loader
//! paths on macOS and Linux). The current working directory is never searched
//! on its own, so a file next to a video cannot inject a library.

use std::ffi::c_ulong;
use std::fmt;
use std::path::{Path, PathBuf};

/// The libmpv client API major version Nitrate is written against.
pub const REQUIRED_API_MAJOR: u32 = 2;

/// File names tried, in order, on the current platform.
pub fn candidate_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["libmpv-2.dll", "mpv-2.dll"]
    } else if cfg!(target_os = "macos") {
        &["libmpv.2.dylib", "libmpv.dylib"]
    } else {
        &["libmpv.so.2", "libmpv.so"]
    }
}

/// A libmpv client API version, `major.minor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ApiVersion {
    pub major: u32,
    pub minor: u32,
}

impl ApiVersion {
    /// Decodes `mpv_client_api_version()`: `major << 16 | minor`.
    /// Generic because C's `unsigned long` is 32 bits on Windows and 64 bits
    /// on macOS and Linux.
    pub fn from_raw(raw: impl Into<u64>) -> Self {
        let raw: u64 = raw.into();
        Self { major: ((raw >> 16) & 0xffff) as u32, minor: (raw & 0xffff) as u32 }
    }
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Why libmpv could not be used.
#[derive(Debug)]
pub enum LoadError {
    /// No candidate file could be loaded. Lists every attempt and its error.
    NotFound { tried: Vec<(String, String)> },
    /// The library loaded but lacks a required function.
    MissingSymbol { path: String, symbol: &'static str, error: String },
    /// The library's client API major version is not the one Nitrate needs.
    WrongVersion { path: String, found: ApiVersion },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotFound { tried } => {
                write!(f, "libmpv was not found. Tried:")?;
                for (name, error) in tried {
                    write!(f, "\n  {name}: {error}")?;
                }
                Ok(())
            }
            LoadError::MissingSymbol { path, symbol, error } => {
                write!(f, "{path} is not a usable libmpv: {symbol} is missing ({error})")
            }
            LoadError::WrongVersion { path, found } => {
                write!(f, "{path} has libmpv client API {found}; Nitrate needs {REQUIRED_API_MAJOR}.x")
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// A loaded libmpv.
pub struct Libmpv {
    // Kept alive for as long as any function pointer taken from it is used.
    _library: libloading::Library,
    path: String,
    api_version: ApiVersion,
}

impl Libmpv {
    /// Loads libmpv from the executable's folder or the system search path.
    pub fn load() -> Result<Self, LoadError> {
        let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
        let mut attempts: Vec<PathBuf> = Vec::new();
        for name in candidate_names() {
            if let Some(dir) = &exe_dir {
                attempts.push(dir.join(name));
            }
        }
        // Bare names go through the platform loader's own search.
        attempts.extend(candidate_names().iter().map(PathBuf::from));
        Self::load_from(&attempts)
    }

    /// Tries each path in order and returns the first usable libmpv.
    pub fn load_from(paths: &[PathBuf]) -> Result<Self, LoadError> {
        let mut tried = Vec::new();
        for path in paths {
            // A relative path with folders would resolve against the working
            // directory; only absolute paths and bare names are allowed.
            if !path.is_absolute() && path.components().count() != 1 {
                tried.push((path.display().to_string(), "skipped: relative path".to_owned()));
                continue;
            }
            // SAFETY: loading a library runs its initialisers. libmpv's do not
            // touch process state Nitrate relies on; the paths come from the
            // executable's folder or the platform's library search, never from
            // the working directory or from media files.
            match unsafe { libloading::Library::new(path) } {
                Ok(library) => return Self::from_library(library, path.display().to_string()),
                Err(error) => tried.push((path.display().to_string(), error.to_string())),
            }
        }
        Err(LoadError::NotFound { tried })
    }

    fn from_library(library: libloading::Library, path: String) -> Result<Self, LoadError> {
        const SYMBOL: &str = "mpv_client_api_version";
        // SAFETY: `mpv_client_api_version` is declared in libmpv's client.h as
        // `unsigned long mpv_client_api_version(void)`; the type below matches it.
        let raw = unsafe {
            let function = library
                .get::<unsafe extern "C" fn() -> c_ulong>(b"mpv_client_api_version\0")
                .map_err(|e| LoadError::MissingSymbol { path: path.clone(), symbol: SYMBOL, error: e.to_string() })?;
            function()
        };
        let api_version = ApiVersion::from_raw(raw);
        if api_version.major != REQUIRED_API_MAJOR {
            return Err(LoadError::WrongVersion { path, found: api_version });
        }
        Ok(Self { _library: library, path, api_version })
    }

    /// Where the library was loaded from (a full path, or the bare name the
    /// system loader resolved).
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The library's client API version.
    pub fn api_version(&self) -> ApiVersion {
        self.api_version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_client_api_version() {
        let v = ApiVersion::from_raw((2u64 << 16) | 5);
        assert_eq!(v, ApiVersion { major: 2, minor: 5 });
        assert_eq!(v.to_string(), "2.5");
    }

    #[test]
    fn every_platform_has_candidates() {
        let names = candidate_names();
        assert!(!names.is_empty());
        assert!(names.iter().all(|n| !n.contains(['/', '\\'])), "candidates must be bare file names");
    }

    #[test]
    fn a_missing_library_is_an_error_not_a_crash() {
        let dir = std::env::temp_dir().join("nitrate-no-such-dir-7f3a");
        let paths = vec![dir.join("libmpv-missing.dll"), dir.join("libmpv-missing.so")];
        match Libmpv::load_from(&paths) {
            Err(LoadError::NotFound { tried }) => assert_eq!(tried.len(), 2),
            Err(other) => panic!("unexpected error: {other}"),
            Ok(_) => panic!("a missing file loaded"),
        }
    }

    #[test]
    fn relative_paths_with_folders_are_never_loaded() {
        let paths = vec![PathBuf::from("media").join("libmpv-2.dll")];
        match Libmpv::load_from(&paths) {
            Err(LoadError::NotFound { tried }) => assert!(tried[0].1.starts_with("skipped")),
            Err(other) => panic!("unexpected error: {other}"),
            Ok(_) => panic!("a relative path was loaded"),
        }
    }
}
