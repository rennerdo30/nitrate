// Release builds on Windows are GUI-subsystem programs: no console window opens.
// Redirected stdout still carries `--version` (the installer test relies on it).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
Nitrate: a fast media player for Windows, macOS and Linux.

Usage: nitrate [OPTIONS] [FILE]

Options:
  --mpv-info   Show which libmpv Nitrate finds, then exit
  --version    Print the version, then exit
  --help       Print this help, then exit

Playback arrives with the rendering spike; this build only checks its setup.";

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Version,
    Help,
    MpvInfo,
    Open(Option<String>),
    Unknown(String),
}

fn parse(args: &[String]) -> Command {
    let mut file = None;
    for arg in args {
        match arg.as_str() {
            "--version" | "-V" => return Command::Version,
            "--help" | "-h" => return Command::Help,
            "--mpv-info" => return Command::MpvInfo,
            a if a.starts_with('-') => return Command::Unknown(a.to_owned()),
            a => file = Some(a.to_owned()),
        }
    }
    Command::Open(file)
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
        Command::Open(_) => {
            eprintln!("Nitrate {VERSION} cannot play files yet. Run `nitrate --help`.");
            ExitCode::FAILURE
        }
        Command::Unknown(arg) => {
            eprintln!("Unknown option {arg}. Run `nitrate --help`.");
            ExitCode::from(2)
        }
    }
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
        assert_eq!(parse(&args(&["movie.mkv"])), Command::Open(Some("movie.mkv".to_owned())));
        assert_eq!(parse(&args(&[])), Command::Open(None));
        assert_eq!(parse(&args(&["--bogus"])), Command::Unknown("--bogus".to_owned()));
    }
}
