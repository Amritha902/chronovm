# chronovm

**A stack-based bytecode VM you can scrub like a video — rewind, replay, and ask any value _why_ it is what it is.**

### ▶ [Try it live in your browser →](https://amritha902.github.io/chronovm/)

No install required: the entire VM is compiled to WebAssembly and runs client-side in a CRT-phosphor terminal UI. Drag the timeline, click a variable to trace its cause, or search the whole run for `depth >= 4`.

---

chronovm is a small virtual machine with a twist: it records **every** step it executes into an immutable trace. That one design choice turns a debugger into a time machine — you can drag execution backwards and forwards through time, and because the whole run is recorded up front, seeking to any step is instant.

## Headline features

### 1. Time-travel scrubbing — O(1) rewind

The interpreter snapshots a full `Frame` after every instruction, so the debugger is a pure function of a single integer cursor. Moving backward is exactly as cheap as moving forward — both are just array indexing, `&trace.frames[n]`. Scrub a million-step run to any point with no replay. Press `space` and it plays forward like a video; hold `←` and watch the stack _un-compute_ as the highlighted instruction walks backward through the source.

### 2. The causal jump — "why is this value what it is?"

Point at any variable, press `w`, and chronovm answers by walking the data **backwards** to the exact instruction that produced it. Every value on the stack carries the step that created it, and that provenance flows _through_ variables on load/store — so the causal chain threads through arithmetic, through named variables, and even **across function calls and recursion**, following the data rather than the control flow. Press `↑`/`↓` to walk each cause; every hop teleports the whole machine to that moment in time.

```
┌─ why is `acc` == 120? ──────────────────────────────────────────────────┐
│ ▶ step   41  store acc                                                   │
│ · step   40  mul  ⇒ 120                                                  │
│ · step   38  load i  ⇒ 5                                                 │
│ · step   33  store acc                                                   │
└──────────────────────────────────────────────────────────────────────────┘
```

### 3. The reverse-unwinding call stack

Every `call` gets its own scope with independent locals, and every frame snapshots the whole call stack. So a recursive `fact(n)` shows a call-stack panel that grows `main() → fact() → fact() → …` five deep and collapses as you scrub — each frame carrying its own `n`. You can inspect the live call stack at _any_ historical moment, not just the present.

### Plus: search across time

Press `/` and type a condition — chronovm teleports to the first step where it holds, and `n`/`N` walk every match:

| Query        | Jumps to…                                     |
| ------------ | --------------------------------------------- |
| `acc > 100`  | the first step a variable crosses a threshold |
| `n == 0`     | the moment a variable hits a value            |
| `depth >= 4` | the first time recursion gets that deep       |
| `top < 0`    | when the top of the stack goes negative       |
| `fault`      | the step where execution faulted              |

Operators: `== != < > <= >=`. Variable lookups scan the call stack inward, so a local inside a function is findable while you're in that call.

## Quick start

```sh
cargo run -- debug examples/recursive.cvm   # open the time-travel debugger (TUI)
cargo run -- run   examples/fib.cvm         # run headless, just print output
```

## Examples

Every example is a plain-text `.cvm` program. Run any with `cargo run -- debug examples/<name>.cvm`.

| Example         | What it does                                     | Output                                     |
| --------------- | ------------------------------------------------ | ------------------------------------------ |
| `factorial.cvm` | Iterative `5!` — the flagship "why?" demo        | `120`                                      |
| `fib.cvm`       | Fibonacci sequence, first 10 terms               | `0 1 1 2 3 5 8 13 21 34`                   |
| `recursive.cvm` | Recursive `fact(5)` — the call-stack showpiece   | `120`                                      |
| `gcd.cvm`       | Euclid's GCD of 48 and 18                        | `6`                                        |
| `sum_to_n.cvm`  | Sum of 1..10                                      | `55`                                       |
| `power.cvm`     | Integer power `2 ^ 10`                           | `1024`                                     |
| `collatz.cvm`   | Collatz sequence from 7                           | `7 22 11 34 17 52 26 13 40 20 10 5 16 8 4 2 1` |
| `countdown.cvm` | A gentle first program, counting down            | `5 4 3 2 1`                                |
| `buggy.cvm`     | Divides by a counter that hits zero — scrub to the fault | `33 50 100` → division-by-zero fault |

## Debugger key map

| Key            | Action                                                 |
| -------------- | ------------------------------------------------------ |
| `←` / `→`      | Step one instruction back / forward                    |
| `[` / `]`      | Leap 25 steps                                          |
| `space`        | Play / pause auto-advance (replays like a video)       |
| `home` / `end` | Jump to the start / end of the run                     |
| `tab`          | Pick a variable                                        |
| `w`            | **Why?** — jump to the cause of the selected value     |
| `↑` / `↓`      | Walk the causal chain (while the panel is open)        |
| `/`            | **Search time** — jump to a step matching a condition  |
| `n` / `N`      | Next / previous search match                           |
| `q`            | Quit                                                   |

## The language in 30 seconds

Programs are plain text (`.cvm`), assembled by a tiny two-pass assembler with labels and named, frame-scoped variables:

```asm
    push 5
    store n          ; n = 5
    push 1
    store acc
loop:
    load acc
    load n
    mul
    store acc        ; acc = acc * n
    ; ... (see examples/factorial.cvm)
```

**Instructions:** `push pop dup swap` · `add sub mul div mod neg` · `eq lt gt le ge not` · `load store` · `jmp jz jnz` · `call ret` · `print halt`.

Functions are just labels you `call`; each call gets its own locals, and arguments and return values travel on the shared value stack. Arithmetic is checked, so overflow and division by zero become clean VM faults you can scrub back to — not crashes. See [`LANGUAGE.md`](LANGUAGE.md) for the full reference.

## Architecture

One UI-agnostic Rust core drives two front ends without duplicating any logic:

```
src/isa.rs        instruction set + assembled Program
src/assembler.rs  two-pass .cvm assembler (labels, named vars)
src/vm.rs         the RECORDING VM — one immutable Frame per step;
                  every value carries provenance + the causal engine (BFS)
src/query.rs      timeline search language (acc > 100, depth >= 4, fault…)
src/tui.rs        terminal debugger (ratatui)   — feature "tui"
src/wasm.rs       wasm-bindgen Session → JSON    — feature "wasm"
docs/             the browser UI (index.html) + generated wasm (pkg/)
```

The crate is a **lib + bin**: `lib.rs` exposes the core, and the `chronovm` binary (`main.rs`) is gated behind the `tui` feature. A Cargo feature split (`tui` vs `wasm`) keeps the VM, causal engine, and query language below the gates, so the terminal and browser run the _exact same_ recorded `Trace`. The wasm build turns `tui` off and `wasm` on; the browser is just another view over the model.

Read the full deep-dive in [`ARCHITECTURE.md`](ARCHITECTURE.md) — recording model, value provenance, the causal BFS, per-frame scopes, and the wasm bridge.

## Build & test

```sh
cargo build --release   # the terminal debugger
cargo test              # VM, assembler, causal engine, recursion, rendering
```

## The browser build

The VM core compiles to `wasm32-unknown-unknown` and is exposed to JavaScript via a thin `Session` bridge — the page is plain HTML/JS, styled as a CRT phosphor terminal (scanlines, bloom, boot sequence, a phosphor flash on each step).

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli             # once
./build-web.sh                             # compiles wasm + generates docs/pkg/
(cd docs && python3 -m http.server 8080)   # ES modules + wasm need http://
```

The [live demo](https://amritha902.github.io/chronovm/) is this same `docs/` folder served by GitHub Pages.

## License

[MIT](LICENSE)
