//! Integration tests that lock in the observable behaviour of every bundled
//! example program in `examples/`.
//!
//! Each test assembles the real `.cvm` source (embedded at compile time with
//! `include_str!`), records a full execution with the public VM, and asserts on
//! the final frame's `output` and `error`. If someone changes an example
//! program — or accidentally regresses the assembler or VM — one of these tests
//! turns red with a precise before/after.

use chronovm::assembler::assemble;
use chronovm::vm::record;

/// Assemble and run an example, returning `(trimmed_output, error)` from the
/// final recorded frame. Panics with a readable message if assembly fails, so a
/// broken example points straight at itself.
fn run(name: &str, src: &str) -> (String, Option<String>) {
    let program = assemble(src).unwrap_or_else(|e| panic!("{name} failed to assemble: {e}"));
    let trace = record(program);
    let last = &trace.frames[trace.last()];
    (last.output.trim().to_string(), last.error.clone())
}

/// The printed lines of a program, re-joined with single spaces. Handy for the
/// examples that emit one value per line but read naturally as a sequence.
fn as_sequence(output: &str) -> String {
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn factorial() {
    let (output, error) = run("factorial.cvm", include_str!("../examples/factorial.cvm"));
    assert_eq!(output, "120");
    assert_eq!(error, None);
}

#[test]
fn fib() {
    let (output, error) = run("fib.cvm", include_str!("../examples/fib.cvm"));
    assert_eq!(as_sequence(&output), "0 1 1 2 3 5 8 13 21 34");
    assert_eq!(error, None);
}

#[test]
fn recursive() {
    let (output, error) = run("recursive.cvm", include_str!("../examples/recursive.cvm"));
    assert_eq!(output, "120");
    assert_eq!(error, None);
}

#[test]
fn gcd() {
    let (output, error) = run("gcd.cvm", include_str!("../examples/gcd.cvm"));
    assert_eq!(output, "6");
    assert_eq!(error, None);
}

#[test]
fn sum_to_n() {
    let (output, error) = run("sum_to_n.cvm", include_str!("../examples/sum_to_n.cvm"));
    assert_eq!(output, "55");
    assert_eq!(error, None);
}

#[test]
fn power() {
    let (output, error) = run("power.cvm", include_str!("../examples/power.cvm"));
    assert_eq!(output, "1024");
    assert_eq!(error, None);
}

#[test]
fn countdown() {
    let (output, error) = run("countdown.cvm", include_str!("../examples/countdown.cvm"));
    assert_eq!(as_sequence(&output), "5 4 3 2 1");
    assert_eq!(error, None);
}

#[test]
fn collatz() {
    let (output, error) = run("collatz.cvm", include_str!("../examples/collatz.cvm"));
    let seq = as_sequence(&output);
    assert!(
        seq.starts_with("7 22 11"),
        "unexpected collatz start: {seq}"
    );
    assert!(seq.ends_with('1'), "collatz should terminate at 1: {seq}");
    assert_eq!(error, None);
}

#[test]
fn buggy() {
    let (_output, error) = run("buggy.cvm", include_str!("../examples/buggy.cvm"));
    let error = error.expect("buggy.cvm is expected to fault");
    assert!(
        error.contains("division by zero"),
        "expected a division-by-zero fault, got: {error}"
    );
}
