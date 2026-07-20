# chronovm — live progress

A running status doc. Updated as we build. (Backlog lives in `NIGHT_PLAN.md`.)

## What the project is

**chronovm** is a stack-based bytecode VM with a **time-travel debugger**. It
records *every* execution step, so you can scrub backwards and forwards through
a program like a video. Two things make it special:

- **Causal "why?"** — click any value and it jumps to the exact step that
  produced it, tracing the data backwards through arithmetic, variables, and
  even recursion.
- **Reverse-unwinding call stack** — functions have their own locals, so a
  recursive `fact(n)` shows a call stack that grows and collapses as you scrub.

It runs two ways from **one shared Rust core**:
- a terminal debugger (ratatui), and
- a **browser build** (WebAssembly) with a CRT-phosphor UI — live at
  https://amritha902.github.io/chronovm/

## Architecture at a glance

```
src/isa.rs        instruction set + assembled Program
src/assembler.rs  two-pass .cvm assembler (labels, named vars)
src/vm.rs         the RECORDING VM — one immutable Frame per step;
                  every value carries provenance (the step that made it)
src/query.rs      timeline search language (acc > 100, depth >= 4, fault…)
src/tui.rs        terminal debugger (ratatui)
src/wasm.rs       wasm-bindgen Session exposing the core to the browser
docs/             the browser UI (index.html) + generated wasm (pkg/)
```

The core (`isa/assembler/vm/query`) is UI-agnostic and compiles to both native
and `wasm32`. The browser and terminal run the *same* VM and causal engine.

## Status right now

- ✅ VM + assembler + recording (one frame/step, O(1) rewind)
- ✅ Causal provenance engine (the "why?" jump)
- ✅ Functions + reverse call stack (per-frame locals)
- ✅ Timeline search / watchpoint language
- ✅ Terminal TUI debugger
- ✅ WebAssembly browser build + live GitHub Pages demo
- ✅ CRT phosphor UI (scanlines, bloom, boot sequence, phosphor-flash on step)
- ✅ Integer-overflow crash fixed (checked arithmetic → clean fault)
- ✅ Tests green + clippy clean
- ⏳ In progress: see the build log below and `NIGHT_PLAN.md`

## Health

- Tests: **18 passing**
- Clippy: **clean**
- Examples verified: factorial→120, fib→0 1 1 2 3 5 8 13 21 34, recursive→120
- Live demo: up

## Build log (newest first)

- **CRT deepening** — screen bezel/curvature, phosphor-persistence flash on
  each step, slow refresh sweep, typewriter boot, mobile polish.
- **CRT phosphor redesign** — replaced the generic dark-gradient look with a
  monochrome-green terminal aesthetic.
- **WASM browser build** — VM core compiled to wasm; browser debugger; live on
  GitHub Pages.
- **Timeline search** — jump to the step a condition first holds.
- **Functions + reverse call stack** — per-frame locals; recursion.
- **Time-travel TUI + causal engine** — the original core.
