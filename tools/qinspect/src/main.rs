//! qinspect: inspect game data and renderer geometry.
//!
//! `qinspect progs ..`      QuakeC disassembler and summary (progs.rs)
//! `qinspect [portals] FILE..` renderer-matched teleporter sheets (portals.rs)

mod portals;
mod progs;

use std::process::ExitCode;

fn main() -> ExitCode {
    // Exit quietly when piped into `head` instead of panicking on EPIPE.
    std::panic::set_hook(Box::new(|i| {
        if !i.to_string().contains("Broken pipe") {
            eprintln!("{i}");
        }
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("progs") => progs::run(&args),
        Some("portals") => portals::run(&args[1..]),
        _ => portals::run(&args),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
