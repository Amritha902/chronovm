# chronovm

**A stack-based bytecode VM you can scrub like a video.**

### ▶ [Try it live in your browser →](https://amritha902.github.io/chronovm/)

No install — the whole VM runs as WebAssembly. Drag the timeline, click a
variable to ask *why* it holds its value, or search the run for `depth >= 5`.

chronovm is a small virtual machine with a twist: it records *every* step it
executes, so its terminal debugger lets you drag execution **backwards and
forwards through time**. Because the whole run is recorded up front, rewinding
to any step is instant — O(1) — no matter how long the program ran.

Two things make it more than a toy:

- **The causal jump.** Point at any variable, press `w`, and chronovm answers
  *"why is this value what it is?"* by walking the data backwards to the exact
  instruction that produced it — through arithmetic, through other variables,
  and even across function calls.
- **A reverse-unwinding call stack.** Functions get their own local scopes, so a
  recursive `fact(n)` shows a call stack that grows five deep and collapses as
  you scrub — each frame carrying its own `n`.
- **Search across time.** Press `/` and type a condition like `acc > 100`,
  `n == 0`, `depth >= 4`, or `fault`, and chronovm teleports to the exact step it
  first became true. `n` / `N` walk every match.

```
┌─ source · 24 instructions ──────────┐┌─ stack · depth 1 ───────────────┐
│  ▶  14 load acc                      ││ top →      120   (from step 41) │
│     15 load i                        │└─────────────────────────────────┘
│     16 mul                           │┌─ variables · [w] why? ──────────┐
│     17 store acc                     ││ ◆ acc     = 120   (set @ step 41)│
│  ...                                 ││   i       = 6     (set @ step 44)│
└──────────────────────────────────────┘└─────────────────────────────────┘
┌─ why is `acc` == 120? ──────────────────────────────────────────────────┐
│ ▶ step   41  store acc                                                   │
│ · step   40  mul  ⇒ 120                                                  │
│ · step   38  load i  ⇒ 5                                                 │
│ · step   33  store acc                                                   │
└──────────────────────────────────────────────────────────────────────────┘
```

## Quick start

```sh
cargo run -- debug examples/factorial.cvm   # open the time-travel debugger
cargo run -- run   examples/fib.cvm         # run headless, just print output
```

### Debugger keys

| Key            | Action                                             |
| -------------- | -------------------------------------------------- |
| `←` / `→`      | Step one instruction back / forward                |
| `[` / `]`      | Leap 25 steps                                      |
| `space`        | Play / pause auto-advance (replays like a video)   |
| `home` / `end` | Jump to the start / end of the run                 |
| `tab`          | Pick a variable                                    |
| `w`            | **Why?** — jump to the cause of the selected value |
| `↑` / `↓`      | Walk the causal chain (while the panel is open)    |
| `/`            | **Search time** — jump to a step matching a condition |
| `n` / `N`      | Next / previous search match                       |
| `q`            | Quit                                               |

### Searching across time

Press `/` and type a condition:

| Query          | Jumps to…                                          |
| -------------- | -------------------------------------------------- |
| `acc >= 100`   | the first step a variable crosses a threshold      |
| `n == 0`       | the moment a variable hits a value                 |
| `depth >= 4`   | the first time recursion gets that deep            |
| `top < 0`      | when the top of the stack goes negative            |
| `fault`        | the step where execution faulted                   |

Operators: `== != < > <= >=`. Variable lookups scan the call stack inward, so a
local inside a function is findable while you're in that call.

## The demo (90 seconds)

1. `cargo run -- debug examples/factorial.cvm`. It opens **parked on the final
   frame** — `acc == 120`.
2. Grab the timeline: hold `←`. Watch the stack un-compute and the highlighted
   instruction walk *backwards* through the source.
3. Press `space`. It replays forward like a video.
4. `tab` to select `acc`, then press **`w`**. chronovm teleports you to the
   `mul` that produced 120 and lists the whole causal chain. Press `↑`/`↓` to
   walk each cause — every hop moves the whole machine to that moment in time.
5. Open `examples/buggy.cvm`, press `end`, then `←` once: you're standing on the
   exact step *before* a division-by-zero fault.

### The recursion demo (the call stack)

`cargo run -- debug examples/recursive.cvm` runs a **recursive** factorial.
Scrub forward and the call-stack panel grows `main() → fact() → fact() → …` five
deep, each frame showing its own `n`; scrub back and it unwinds. Park at the end,
select `result`, press `w`, and the causal chain threads back through every
multiplication *across the recursive frames*.

## The language

Programs are plain text (`.cvm`), assembled by a tiny two-pass assembler with
labels and named variables:

```asm
    push 5
    store n         ; n = 5
    push 1
    store acc
loop:
    load acc
    load n
    mul
    store acc        ; acc = acc * n
    ; ... (see examples/factorial.cvm)
```

**Instructions:** `push pop dup swap` · `add sub mul div mod neg` ·
`eq lt gt le ge not` · `load store` · `jmp jz jnz` · `call ret` · `print halt`.

Functions are just labels you `call`. Each call gets its own locals; arguments
and return values are passed on the shared value stack. See
[`examples/recursive.cvm`](examples/recursive.cvm).

## How it works

- **`isa.rs`** — the instruction set and the assembled `Program`.
- **`assembler.rs`** — two-pass assembler; pass one maps labels to indices,
  pass two resolves jump targets.
- **`query.rs`** — the timeline search language (`parse` + `Predicate::holds`),
  evaluated against any recorded frame.
- **`vm.rs`** — the recording VM. Every value on the stack carries the *step
  that produced it*. When a value is stored into a variable and later loaded,
  that provenance flows through the variable — which is what makes causal
  queries work across variable (and function) boundaries. Each `call` pushes a
  `Scope` with its own locals, so recursion is faithfully recorded.
  `record()` returns an immutable `Trace` of one `Frame` per step.
- **`tui.rs`** — the ratatui UI. The entire display is a pure function of one
  integer, `cursor`, so time-travel is just changing which frame we render.

## The browser build

The VM core is UI-agnostic and compiles to WebAssembly, so the browser
debugger ([`docs/`](docs/index.html)) runs the *exact same* VM, causal engine,
and query language as the terminal — no logic is duplicated. `src/wasm.rs`
exposes a `Session` via wasm-bindgen; the page is plain HTML/JS.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli     # once
./build-web.sh                     # compiles wasm + generates docs/pkg/
(cd docs && python3 -m http.server 8080)   # ES modules + wasm need http://
```

The live demo above is this same `docs/` folder served by GitHub Pages.

## Build & test

```sh
cargo build --release   # the terminal debugger
cargo test              # 17 tests: VM, assembler, causal, recursion, rendering
```

## License

MIT
