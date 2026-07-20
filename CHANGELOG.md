# Changelog

All notable changes to chronovm are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] — 0.1.0

First public preview of chronovm: a stack-based bytecode VM with a time-travel
debugger, a causal provenance engine, a terminal TUI, and a WebAssembly build
that runs the whole debugger in the browser.

### Added

- **Bytecode VM, assembler, and recording core.** A stack-based virtual machine
  with a compact instruction set, a text assembler that turns human-readable
  programs into bytecode, and a recorder that captures every execution step so
  the full run history can be replayed.
- **Time-travel debugger (TUI).** A terminal interface for stepping a program
  forward and backward through its recorded history, inspecting the stack,
  memory, and instruction pointer at any point in time.
- **Causal provenance engine.** Every value can be traced to the instructions
  and prior values that produced it, so you can ask "where did this come from?"
  and walk the causal chain backward through the run.
- **Functions and a reverse-unwinding call stack.** Function calls and returns
  are recorded, and the call stack can be unwound in reverse to see how
  execution arrived at the current frame.
- **Timeline search.** Jump directly to the step at which a condition first
  becomes true, instead of scrubbing the timeline by hand.
- **WebAssembly browser build and live demo.** The debugger compiles to WASM and
  runs entirely client-side. A hosted demo is available at
  <https://amritha902.github.io/chronovm/>.
- **Documentation, example gallery, and language reference.** A gallery of
  runnable example programs, a reference for the assembly language, and project
  documentation covering the VM and the browser demo.
- **Continuous integration.** CI configuration plus formatting and lint checks
  to keep the build green.
- **Licensing.** Project LICENSE added.

### Changed

- **CRT phosphor UI redesign.** The web front end was fully redesigned as a CRT
  phosphor terminal, with a refined visual system, screen curvature,
  phosphor-flash and sweep effects, a boot sequence, and timeline track markers.
- **Motion and mobile polish.** CRT motion was finalized and the layout was
  tuned for mobile screens.

### Fixed

- **Integer-overflow crash.** Fixed a crash caused by integer overflow during
  execution.

[Unreleased]: https://github.com/Amritha902/chronovm
