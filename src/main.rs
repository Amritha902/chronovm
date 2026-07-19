//! chronovm — a stack-based bytecode VM with a time-travel debugger.
//!
//! Usage:
//!   chronovm debug <file.cvm>   Record an execution and open the time-travel UI
//!   chronovm run   <file.cvm>   Run headless and print the program's output
//!   chronovm help               Show this message
//!
//! With a single file argument and no subcommand, `debug` is assumed.

use chronovm::{assembler, tui, vm};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let (command, path) = match args.as_slice() {
        [] => {
            usage();
            return ExitCode::FAILURE;
        }
        [one] if one == "help" || one == "-h" || one == "--help" => {
            usage();
            return ExitCode::SUCCESS;
        }
        [file] => ("debug", file.as_str()),
        [cmd, file, ..] => (cmd.as_str(), file.as_str()),
    };

    match run(command, path) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("chronovm: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: &str, path: &str) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read `{path}`: {e}"))?;
    let program = assembler::assemble(&src)?;

    match command {
        "debug" => {
            let trace = vm::record(program);
            tui::run(trace)?;
            Ok(ExitCode::SUCCESS)
        }
        "run" => {
            let trace = vm::record(program);
            let last = &trace.frames[trace.last()];
            print!("{}", last.output);
            if let Some(err) = trace.faulted() {
                eprintln!("fault at step {}: {err}", trace.last());
                return Ok(ExitCode::FAILURE);
            }
            Ok(ExitCode::SUCCESS)
        }
        other => {
            eprintln!("chronovm: unknown command `{other}`\n");
            usage();
            Ok(ExitCode::FAILURE)
        }
    }
}

fn usage() {
    eprintln!(
        "chronovm — a bytecode VM you can scrub like a video\n\
         \n\
         USAGE:\n    \
         chronovm debug <file.cvm>   record an execution and open the time-travel debugger\n    \
         chronovm run   <file.cvm>   run headless and print output\n    \
         chronovm help               show this message\n\
         \n\
         A single file with no command defaults to `debug`.\n\
         \n\
         In the debugger: ←/→ step · [ ] leap · space play · tab pick a variable · w \"why?\" · q quit"
    );
}
