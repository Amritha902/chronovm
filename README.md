# chronovm

**A stack-based bytecode VM you can scrub like a video — rewind, replay, and ask any value _why_ it is what it is.**

### ▶ [Try it live in your browser →](https://amritha902.github.io/chronovm/)

No install required: the entire VM is compiled to WebAssembly and runs client-side in a CRT-phosphor terminal UI. Drag the timeline, click a variable to trace its cause, click a stack slot to jump to what produced it, or search the whole run for `depth >= 4`.

---

chronovm is a small virtual machine with a twist: it records **every** step it executes into an immutable trace. That one design choice turns a debugger into a time machine — you can drag execution backwards and forwards through time, and because the whole run is recorded up front, seeking to any step is instant.

One UI-agnostic Rust core drives **two front ends**: a [ratatui](https://ratatui.rs) **terminal** debugger and a **WebAssembly browser** debugger — both are thin views over the same recorded trace, so they behave identically.

## Headline features

### 1. Time-travel scrubbing — O(1) rewind

The interpreter snapshots a full `Frame` after every instruction, so the debugger is a pure function of a single integer cursor. Moving backward is exactly as cheap as moving forward — both are just array indexing, `&trace.frames[n]`. Scrub a long run to any point with no replay. Press `space` and it plays forward like a video; hold `←` and watch the stack _un-compute_ as the highlighted instruction walks backward through the source.

### 2. The causal jump — "why is this value what it is?"

Point at any variable, press `w` (or click it in the browser), and chronovm answers by walking the data **backwards** to the exact instruction that produced it. Every value on the stack carries the step that created it, and that provenance flows _through_ variables on load/store **and through linear memory** — so the causal chain threads through arithmetic, through named variables, through arrays in memory, and even **across function calls and recursion**, following the data rather than the control flow. Press `↑`/`↓` to walk each cause; every hop teleports the whole machine to that moment in time.

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

Type a condition and chronovm teleports to the first step where it holds, then walks every match:

| Query        | Jumps to…                                     |
| ------------ | --------------------------------------------- |
| `acc > 100`  | the first step a variable crosses a threshold |
| `n == 0`     | the moment a variable hits a value            |
| `depth >= 4` | the first time recursion gets that deep       |
| `top < 0`    | when the top of the stack goes negative       |
| `fault`      | the step where execution faulted              |

Operators: `== != < > <= >=` (`=` also parses). `depth` is call-stack depth and `top` is the top-of-stack value; any other name is looked up as a variable, scanning the call stack inward so a local inside a function is findable while you're in that call.

## The browser debugger

The [live demo](https://amritha902.github.io/chronovm/) is the same recorded trace as the terminal, rendered as a CRT phosphor terminal (scanlines, bloom, a typewriter boot sequence, a phosphor flash on each step). Beyond scrubbing, causal "why?", and search, it adds:

- **Drag-slider timeline** with play / pause and single-step, plus **step-over** (skip a call's internals) and **step-out** (run to the end of the current function).
- **Click a local → "why?"** — trace it back to the step that produced it.
- **Click a stack value → jump** to the exact step that produced it.
- **Timeline search** with fault, match, and causal-chain markers rendered as ticks on the scrubber.
- **Live memory panel** — linear memory shown as a cell grid; cells that changed since the previous step glow.
- **Breakpoints** — click a source line to toggle one; run to the next / previous breakpoint, and auto-play pauses when it lands on one.
- **Diff between two steps** — `mark` a step (A), scrub to another (B), and see exactly which variables, memory cells, stack slots, and output changed.
- **Watch expressions** — watch a variable and see its whole value history as a **sparkline** with the current step marked.
- **Share-URL** — the `share` button encodes the running program into the link (`#p=…`); opening a shared link reproduces it.
- **Help overlay** — press `?` (or the `?` button) for the full key map and click interactions.
- **Built-in examples** — one-click buttons (recursive factorial, iterative factorial, fibonacci, div-by-zero fault, array sum, bubble sort) plus a free-form editor: paste any `.cvm` program and `record` it.

Browser keys: `←`/`→` step · `space` play/pause · `/` focus search · `?` help.

## 🎙 Talk to it — a debugger you can hold a conversation with

Click the mic (bottom-right) and **ask the debugger about your program out loud**.
It answers in voice *and* text, and drives the UI while it does:

> **“why is result 120?”** → *“result is 120. Here is why: step 74, store result;
> then step 70, mul produced 120…”* — and it opens the causal panel, draws the
> provenance arrows, and jumps you to the cause.
>
> **“why did it crash?”** → jumps to the faulting step and traces the bad operand
> back to where it came from.

Other things it understands: `what's on the stack` · `what's in memory` ·
`go to step 40` · `where am I` · `deepest recursion` · `what does this program do` ·
`what did it print` · `play` / `pause` / `start` / `end` · `load bubble sort` · `help`.

**No API key and no backend.** The "brain" is local: it maps what you say onto the
real causal engine and the recorded trace, so it works offline on the static site.
Speech uses the browser's built-in Web Speech API; if your browser doesn't support
speech input (e.g. Firefox), the console gracefully falls back to typing — and
still speaks its answers back. Use the 🔊 button to mute replies.

## Quick start (terminal)

```sh
cargo run -- debug examples/recursive.cvm   # open the time-travel debugger (TUI)
cargo run -- run   examples/fib.cvm         # run headless, just print output
```

`chronovm run` prints the program's output to stdout; if the program faults it reports the faulting step on stderr and exits non-zero. A single file with no subcommand defaults to `debug`. `chronovm help` prints usage.

## Examples

Every example is a plain-text `.cvm` program in [`examples/`](examples/). Run any with `cargo run -- debug examples/<name>.cvm`. Outputs below are locked in by the integration suite in [`tests/examples.rs`](tests/examples.rs).

| Example              | What it does                                          | Output                                            |
| -------------------- | ----------------------------------------------------- | ------------------------------------------------- |
| `factorial.cvm`      | Iterative `5!` — the flagship "why?" demo             | `120`                                             |
| `fib.cvm`            | Fibonacci sequence, first 10 terms                    | `0 1 1 2 3 5 8 13 21 34`                           |
| `recursive.cvm`      | Recursive `fact(5)` — the call-stack showpiece        | `120`                                             |
| `gcd.cvm`            | Euclid's GCD of 48 and 18                             | `6`                                               |
| `sum_to_n.cvm`       | Sum of `1..10`                                        | `55`                                              |
| `power.cvm`          | Integer power `2 ^ 10`                                | `1024`                                            |
| `collatz.cvm`        | Collatz sequence from 7                               | `7 22 11 34 17 52 26 13 40 20 10 5 16 8 4 2 1`    |
| `countdown.cvm`      | A gentle first program, counting down                | `5 4 3 2 1`                                        |
| `array_sum.cvm`      | Sum `[5 2 8 1 9]` held in linear memory (`mstore`/`mload`) | `25`                                          |
| `reverse_array.cvm`  | Write an array to memory, print it back reversed      | `5 4 3 2 1`                                        |
| `array_max.cvm`      | Scan an array in memory for its maximum               | `9`                                               |
| `bubble_sort.cvm`    | In-place bubble sort of an array in memory            | `1 2 5 8 9`                                        |
| `sieve.cvm`          | Sieve of Eratosthenes, primes below 30               | `2 3 5 7 11 13 17 19 23 29`                        |
| `fib_memo.cvm`       | Memoized Fibonacci using linear memory as a cache     | `55`                                              |
| `buggy.cvm`          | Divides `100` by a counter that hits zero             | `33 50 100`, then a division-by-zero fault        |

## Terminal key map

| Key                       | Action                                                 |
| ------------------------- | ------------------------------------------------------ |
| `←` / `→` (or `h` / `l`)  | Step one instruction back / forward                    |
| `[` / `]`                 | Leap 25 steps back / forward                           |
| `home` / `end` (or `g` / `G`) | Jump to the start / end of the run                 |
| `space`                   | Play / pause auto-advance (replays like a video)       |
| `tab` / `↓` / `j`         | Select the next variable (`shift+tab` selects previous) |
| `↑` / `k`                 | Select the previous variable                           |
| `w` / `Enter`             | **Why?** — jump to the cause of the selected variable  |
| `↑` / `↓` (causal panel)  | Walk the causal chain; `←` / `→` step; `Esc` / `q` close |
| `/`                       | **Search time** — jump to the first step matching a condition |
| `n` / `N`                 | Next / previous search match                           |
| `Esc`                     | Clear an active search, otherwise quit                 |
| `q`                       | Quit                                                   |

In the browser, the same ideas are **click interactions**: click a local for "why?", click a stack value to jump to its origin, click a source line to toggle a breakpoint, and click a step in the causal panel to teleport there.

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

**Instructions**

| Group            | Opcodes |
| ---------------- | ------- |
| Stack            | `push N` · `pop` · `dup` · `swap` |
| Arithmetic       | `add` · `sub` · `mul` · `div` · `mod` · `neg` |
| Comparison/logic | `eq` · `lt` · `gt` · `le` · `ge` · `not` |
| Variables        | `load NAME` · `store NAME` (frame-scoped locals) |
| Control flow     | `jmp L` · `jz L` · `jnz L` · `call L` · `ret` |
| Linear memory    | `mstore` — `( value addr -- )` · `mload` — `( addr -- value )` |
| I/O / halt       | `print` · `halt` |

Functions are just labels you `call`; each call gets its own locals, and arguments and return values travel on the shared value stack. **Linear memory** is a flat block of integer cells at addresses `0..=65535`, auto-zeroed at start and shared across all frames — the natural place for arrays (see `array_sum`, `bubble_sort`, `sieve`, `fib_memo`). Arithmetic is checked, so overflow, division/modulo by zero, undefined variables, out-of-bounds memory, and runaway loops become clean VM **faults** you can scrub back to — not crashes. See [`LANGUAGE.md`](LANGUAGE.md) for the full reference and fault list.

## Architecture

One UI-agnostic Rust core drives both front ends without duplicating any logic:

```
src/isa.rs        instruction set + assembled Program
src/assembler.rs  two-pass .cvm assembler (labels, named vars)
src/vm.rs         the RECORDING VM — one immutable Frame per step;
                  every value carries provenance + the causal engine (BFS)
src/query.rs      timeline search language (acc > 100, depth >= 4, fault…)
src/tui.rs        terminal debugger (ratatui)    — feature "tui"
src/wasm.rs       wasm-bindgen Session → JSON     — feature "wasm"
docs/             the browser UI (index.html) + generated wasm (pkg/)
```

The crate is a **lib + bin**: `lib.rs` exposes the core, and the `chronovm` binary (`main.rs`) is gated behind the `tui` feature. A Cargo feature split (`tui` vs `wasm`) keeps the VM, causal engine, and query language below the gates, so the terminal and browser run the _exact same_ recorded `Trace`. The wasm build turns `tui` off and `wasm` on; the browser is just another view over the model.

Read the full deep-dive in [`ARCHITECTURE.md`](ARCHITECTURE.md) — recording model, value provenance, the causal BFS, per-frame scopes, the query language, and the wasm bridge. See also [`LANGUAGE.md`](LANGUAGE.md), [`DEMO.md`](DEMO.md), [`PITCH.md`](PITCH.md), [`CHANGELOG.md`](CHANGELOG.md), and [`PROGRESS.md`](PROGRESS.md).

## Build & test

```sh
cargo build --release   # the terminal debugger
cargo test              # 22 unit + 15 integration tests
```

The integration suite assembles and records each bundled example and asserts on its exact output, so a regression in the assembler, VM, or an example turns a test red with a precise before/after.

## The browser build

The VM core compiles to `wasm32-unknown-unknown` and is exposed to JavaScript via a thin `Session` bridge — the page is plain HTML/JS, styled as a CRT phosphor terminal.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli             # once, matching the pinned wasm-bindgen version
./build-web.sh                             # compiles wasm + generates docs/pkg/
(cd docs && python3 -m http.server 8080)   # ES modules + wasm need http://
```

The [live demo](https://amritha902.github.io/chronovm/) is this same `docs/` folder served by GitHub Pages.

> Continuous-integration config is parked at [`ci/ci.yml`](ci/ci.yml); moving it into `.github/workflows/` requires the GitHub `workflow` OAuth scope.

## License

[MIT](LICENSE)
