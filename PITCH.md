# chronovm

**A bytecode VM you can scrub like a video — and ask any value *why* it became what it is.**

### ▶ [Live in your browser →](https://amritha902.github.io/chronovm/) · no install

---

## The problem

Every debugger you have ever used goes one direction: forward. You set a
breakpoint, blow past it, and now the only way back is to restart and try to
land one step earlier. And when you finally freeze the frame, the debugger tells
you *what* a variable is — never *why*. "How did `total` become `-1`?" turns into
twenty minutes of manual re-execution, print statements, and squinting at a call
stack that has already collapsed. The information that would answer the question
existed a microsecond ago and the tool threw it away.

## The insight

Don't throw it away. chronovm **records every step it executes** and **tracks the
provenance of every value** — which instruction produced it, and which values fed
that instruction. Once the whole run is captured, two things that are normally
impossible become trivial: moving *backwards* through time, and following
causality *backwards* through data.

## What makes it special

- **O(1) time-travel.** The run is recorded up front, so jumping to step 3 or
  step 30,000 costs the same. Drag the timeline and the stack un-computes,
  variables revert, and the highlighted instruction walks backwards through the
  source. No re-execution, no replaying from the top.
- **The causal jump.** Point at any variable, press `w`, and chronovm answers
  *"why is this value what it is?"* — it teleports to the exact instruction that
  produced it and lists the chain that led there, hop by hop. Each hop moves the
  whole machine to that moment, so you're not reading a stack trace, you're
  standing inside it.
- **Causality that threads through recursion.** The "why" chain doesn't stop at a
  function boundary. Ask why `result == 120` in a recursive factorial and it
  walks back through every `mul`, **across five recursive frames**, to the base
  case that started it.
- **A reverse-unwinding call stack.** Each call gets its own scope. Scrub a
  recursive `fact(n)` and watch `main → fact → fact → …` grow five deep and
  collapse as you rewind — every frame carrying its own `n`.
- **Search across time.** Press `/`, type a condition like `depth >= 5`,
  `acc > 100`, `n == 0`, or `fault`, and chronovm teleports to the first step it
  became true. `n` / `N` walk every match. It's grep for the timeline.
- **Catch the bug *before* it happens.** Load a program that faults, jump to the
  end, step back one — you're parked on the exact instruction before a
  division-by-zero, with the whole state that caused it still on screen.

## It's real, and it runs live

chronovm is a working stack VM with a two-pass assembler, named variables,
functions, and recursion — not a mockup. The same Rust core powers **two
surfaces from one codebase**: a full terminal TUI, and a **WebAssembly** build
that runs the entire VM in the browser with nothing to install. The timeline
scrubbing, the causal jump, and the timeline search you see on the live demo are
the identical engine compiled to wasm. Open the link, drag the slider, click a
variable — it's all executing locally in the tab.

## What I'd build next

- **Bytecode from a real language** — a small front-end so you time-travel over
  code you actually wrote, not hand-assembled `.cvm`.
- **Watchpoints across time** — "jump to the step where `x` last changed," and a
  diff of state between any two moments.
- **Heap and reference provenance** — extend the causal jump through mutable data
  structures, not just scalars on the stack.
- **Shareable traces** — export a recorded run as a link so a teammate can scrub
  the exact failure you saw.
- **Scale to long runs** — snapshot-and-replay checkpoints so multi-million-step
  programs stay O(1) to seek without holding every frame in memory.

---

*A debugger that only steps forward is a book you can only read by burning each
page behind you. chronovm keeps the pages.*
