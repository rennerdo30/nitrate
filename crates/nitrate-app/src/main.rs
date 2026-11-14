// Release builds on Windows are GUI-subsystem programs: no console window opens.
// Redirected stdout still carries `--version` (the installer test relies on it).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod d3d;
#[cfg(windows)]
mod player;
#[cfg(windows)]
mod update;

use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
Nitrate: a fast media player for Windows, macOS and Linux.

Usage: nitrate [OPTIONS] [FILE]

Options:
  --mute           Start muted
  --start <SECS>   Start playing at SECS seconds
  --pause          Open the file paused
  --smoke <SECS>   Play offscreen for SECS seconds, print timings, then exit
  --snapshot <PNG> Offscreen run that saves the final frame with controls
  --hover <F>      With --snapshot: point at the timeline at fraction F (0..1)
  --mpv-info       Show which libmpv Nitrate finds, then exit
  --version        Print the version, then exit
  --help           Print this help, then exit

Keys: Space or K play/pause, Left/Right seek 5 s (Shift: 1 s), J/L 10 s,
Up/Down volume, M mute, F or double click fullscreen, Esc leave fullscreen,
. and , frame step, Ctrl+O open, Ctrl+Shift+U install an available update.";

/// Options for a player run.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Options {
    pub file: Option<String>,
    pub mute: bool,
    pub smoke: Option<f64>,
    pub snapshot: Option<String>,
    pub start: Option<f64>,
    pub pause: bool,
    pub hover: Option<f64>,
}

#[derive(Debug, PartialEq)]
enum Command {
    Version,
    Help,
    MpvInfo,
    Play(Options),
    Bad(String),
}

fn parse(args: &[String]) -> Command {
    let mut options = Options::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--version" | "-V" => return Command::Version,
            "--help" | "-h" => return Command::Help,
            "--mpv-info" => return Command::MpvInfo,
            "--mute" => options.mute = true,
            "--smoke" => match it.next().and_then(|s| s.parse::<f64>().ok()).filter(|s| *s > 0.0 && *s <= 600.0) {
                Some(secs) => options.smoke = Some(secs),
                None => return Command::Bad("--smoke needs a number of seconds (up to 600)".into()),
            },
            "--snapshot" => match it.next() {
                Some(path) => options.snapshot = Some(path.clone()),
                None => return Command::Bad("--snapshot needs a PNG path".into()),
            },
            "--start" => match it.next().and_then(|s| s.parse::<f64>().ok()).filter(|s| s.is_finite() && *s >= 0.0) {
                Some(secs) => options.start = Some(secs),
                None => return Command::Bad("--start needs a number of seconds".into()),
            },
            "--pause" => options.pause = true,
            "--hover" => match it.next().and_then(|s| s.parse::<f64>().ok()).filter(|f| (0.0..=1.0).contains(f)) {
                Some(f) => options.hover = Some(f),
                None => return Command::Bad("--hover needs a fraction from 0 to 1".into()),
            },
            a if a.starts_with("--") => return Command::Bad(format!("Unknown option {a}")),
            a => options.file = Some(a.to_owned()),
        }
    }
    if options.snapshot.is_some() && options.smoke.is_none() {
        options.smoke = Some(3.0);
    }
    Command::Play(options)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Command::Version => {
            println!("nitrate {VERSION}");
            ExitCode::SUCCESS
        }
        Command::Help => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Command::MpvInfo => match nitrate_mpv::Libmpv::load() {
            Ok(mpv) => {
                println!("libmpv {} (client API {})", mpv.path(), mpv.api_version());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        Command::Play(options) => play(options),
        Command::Bad(message) => {
            eprintln!("{message}. Run `nitrate --help`.");
            ExitCode::from(2)
        }
    }
}

#[cfg(windows)]
fn play(options: Options) -> ExitCode {
    match player::run(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            show_error(&error);
            ExitCode::FAILURE
        }
    }
}

#[cfg(windows)]
fn show_error(message: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    use windows::core::HSTRING;
    eprintln!("nitrate: {message}");
    // SAFETY: plain message box with owned strings.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(format!("Nitrate can't start.\n\n{message}")),
            &HSTRING::from("Nitrate"),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn play(_options: Options) -> ExitCode {
    eprintln!("Nitrate {VERSION} plays on Windows so far; macOS and Linux follow (docs/SPEC.md).");
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_the_options() {
        assert_eq!(parse(&args(&["--version"])), Command::Version);
        assert_eq!(parse(&args(&["--mpv-info"])), Command::MpvInfo);
        assert_eq!(
            parse(&args(&["movie.mkv", "--mute"])),
            Command::Play(Options { file: Some("movie.mkv".into()), mute: true, ..Options::default() })
        );
        assert_eq!(
            parse(&args(&["--smoke", "5", "a.mkv"])),
            Command::Play(Options { file: Some("a.mkv".into()), smoke: Some(5.0), ..Options::default() })
        );
        assert!(matches!(parse(&args(&["--smoke"])), Command::Bad(_)));
        assert!(matches!(parse(&args(&["--smoke", "9999"])), Command::Bad(_)));
        assert!(matches!(parse(&args(&["--bogus"])), Command::Bad(_)));
        assert_eq!(
            parse(&args(&["--snapshot", "s.png"])),
            Command::Play(Options { smoke: Some(3.0), snapshot: Some("s.png".into()), ..Options::default() })
        );
        assert_eq!(
            parse(&args(&["--start", "90.5", "--pause", "a.mkv"])),
            Command::Play(Options { file: Some("a.mkv".into()), start: Some(90.5), pause: true, ..Options::default() })
        );
        assert!(matches!(parse(&args(&["--start", "-1"])), Command::Bad(_)));
        assert!(matches!(parse(&args(&["--hover", "1.5"])), Command::Bad(_)));
        assert_eq!(parse(&args(&[])), Command::Play(Options::default()));
    }
}
