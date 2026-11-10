//! `fs-files`: one binary for everything (ADR 0063). Pattern: Facade over the crates that stay
//! libraries.
//!
//! ```text
//! fs-files                          the window (feature `gui`, on by default)
//! fs-files [--socket F] [--size WxH] [--snapshot F.png]
//! fs-files serve                    the server (Cap'n Proto, files.capnp)
//! fs-files --version                the version
//! ```

use std::process::ExitCode;

use fs_files_server::ServeError;

fn main() -> ExitCode {
    // The words as the system gave them: a path need not be UTF-8 (the server reads them as they are).
    let raw: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let text = |i: usize| raw.get(i).and_then(|w| w.to_str());
    match text(0) {
        Some("--version" | "-V") => {
            println!("fs-files {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("serve") => serve(&raw[1..]),
        // `fs-files --socket PATH serve`: the option may stand before the word.
        Some("--socket") if text(2) == Some("serve") => {
            let mut words = vec![raw[0].clone(), raw[1].clone()];
            words.extend(raw[3..].iter().cloned());
            serve(&words)
        }
        None | Some("gui") => window(&lossy(raw.get(1..).unwrap_or_default())),
        Some(_) => window(&lossy(&raw)),
    }
}

fn lossy(words: &[std::ffi::OsString]) -> Vec<String> {
    words
        .iter()
        .map(|w| w.to_string_lossy().into_owned())
        .collect()
}

fn serve(words: &[std::ffi::OsString]) -> ExitCode {
    match fs_files_server::serve(words.iter().cloned()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(ServeError::Usage(m)) => {
            eprintln!("fs-files: {m}");
            ExitCode::from(2)
        }
        Err(ServeError::Failed(m)) => {
            eprintln!("fs-files: {m}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(feature = "gui")]
fn window(words: &[String]) -> ExitCode {
    match fs_files_app::gui::run(words) {
        Ok(()) => ExitCode::SUCCESS,
        Err(m) => {
            eprintln!("fs-files: {m}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(feature = "gui"))]
fn window(_words: &[String]) -> ExitCode {
    eprintln!(
        "fs-files: this build has no window (built with --no-default-features); use `fs-files serve`"
    );
    ExitCode::FAILURE
}
