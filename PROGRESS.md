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

- Tests: **37 passing** (22 unit + 15 integration)
- Clippy: **clean** · rustfmt: **clean**
- Examples verified: factorial→120, fib→…, recursive→120, gcd→6, sum_to_n→55,
  power→1024, collatz→full sequence, countdown→5 4 3 2 1, array_sum→25,
  reverse_array→5 4 3 2 1, array_max→9, bubble_sort→1 2 5 8 9
- Live demo: up
- CI config written (in `ci/ci.yml`; owner activates — see NIGHT_PLAN "Needs owner")

## Build log (newest first)

- **Watch sparklines (web)** — watch a variable and see its whole value history
  as a sparkline with the current step marked.
- **Step-over / step-out (web)** and **diff between two steps** (mark A, scrub,
  see what changed in vars/memory/stack/output).
- **Help overlay (web)** — press `?` for a controls modal (timeline keys + click
  interactions).
- **Breakpoints (web)** — click a source line to set one; run to next/prev
  breakpoint; auto-play pauses on them. Plus a **TUI memory panel** for parity,
  and **sieve** / **fib_memo** memory examples.
- **Memory panel + array examples** — the web UI shows linear memory as a live
  cell grid (changed cells glow); "array sum" / "bubble sort" buttons added.
- **Linear memory opcodes** — `mstore`/`mload` with provenance through memory;
  array_sum/reverse_array/array_max/bubble_sort examples + tests.
- **Provenance viz (web)** — stack slots click to jump to the step that produced
  them; the causal chain shows as markers on the timeline.
- **share-URL** — a "share" button encodes the running program into the link
  (`#p=…`); opening a shared link reproduces it.
- **Docs blitz (parallel agents)** — README overhaul, ARCHITECTURE.md,
  LANGUAGE.md, DEMO.md, PITCH.md, CHANGELOG.md, CI config, rustfmt + `cargo fmt`.
- **Example gallery** — gcd, sum_to_n, power, collatz, countdown + README, all
  hand-verified, plus a 9-test integration suite locking their outputs.
- **CRT deepening** — screen bezel/curvature, phosphor-persistence flash on
  each step, slow refresh sweep, typewriter boot, mobile polish.
- **Timeline markers** (overnight cron) — fault + search-match ticks on the
  scrubber.
- **CRT phosphor redesign** — replaced the generic dark-gradient look with a
  monochrome-green terminal aesthetic.
- **WASM browser build** — VM core compiled to wasm; browser debugger; live on
  GitHub Pages.
- **Timeline search** — jump to the step a condition first holds.
- **Functions + reverse call stack** — per-frame locals; recursion.
- **Time-travel TUI + causal engine** — the original core.
