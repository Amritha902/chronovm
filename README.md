# chronovm

**A stack-based bytecode VM you can scrub like a video.**

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
| `q`            | Quit                                               |

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
- **`vm.rs`** — the recording VM. Every value on the stack carries the *step
  that produced it*. When a value is stored into a variable and later loaded,
  that provenance flows through the variable — which is what makes causal
  queries work across variable (and function) boundaries. Each `call` pushes a
  `Scope` with its own locals, so recursion is faithfully recorded.
  `record()` returns an immutable `Trace` of one `Frame` per step.
- **`tui.rs`** — the ratatui UI. The entire display is a pure function of one
  integer, `cursor`, so time-travel is just changing which frame we render.

## Build & test

```sh
cargo build --release
cargo test
```

## License

MIT
