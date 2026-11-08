//! `fs-files`: one binary for everything (ADR 0063). Pattern: Facade over the crates that stay
//! libraries.
//!
//! ```text
//! fs-files                          the window (feature `gui`, on by default)
//! fs-files [--socket F] [--size WxH] [--snapshot F.png]
//! fs-files serve                    the server (Cap'n Proto, files.capnp)
//! ```

use std::process::ExitCode;

use fs_files_server::ServeError;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("serve") => serve(&args[1..]),
        // `fs-files --socket PATH serve`: the option may stand before the word.
        Some("--socket") if args.get(2).map(String::as_str) == Some("serve") => {
            let mut words = vec!["--socket".to_string(), args[1].clone()];
            words.extend(args[3..].iter().cloned());
            serve(&words)
        }
        None | Some("gui") => window(args.get(1..).unwrap_or_default()),
        Some(_) => window(&args),
    }
}

fn serve(words: &[String]) -> ExitCode {
    match fs_files_server::serve(words.iter().map(Into::into)) {
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
