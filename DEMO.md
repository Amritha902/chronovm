# chronovm — 90-second demo

**One line:** a bytecode VM that records *every* step, so you can scrub execution like a video, ask any variable *"why are you this value?"*, and teleport to any moment in the run — including the instant before a crash.

Two surfaces, same engine: the **live web demo** (no install) and the **terminal TUI**. Pick one for the room; the beats are identical.

---

## Track A — Live web demo (open on the projector)

> **https://amritha902.github.io/chronovm/** — the whole VM is WebAssembly. Nothing to install.

1. **It's already parked on the answer.** The page loads having *already run* a recursive factorial. `fact(5)` is done; `result == 120` sits on screen. "It didn't just compute this — it recorded all 40-odd steps that got here."
2. **Rewind.** Drag the timeline left (or press **←**) and watch the call stack *unwind* — `main → fact → fact → …` collapses frame by frame, each frame carrying its own `n`. Every rewind is instant; the run is recorded up front.
3. **Replay like a video.** Press **space**. Execution plays forward on its own — stack grows, variables update, the highlighted instruction walks the source.
4. **Ask "why?"** Click the **`result`** local. chronovm answers *"why is `result == 120`?"* — it jumps to the `mul` that produced it and lists the causal chain, threading backwards through every multiplication **across the recursive frames**.
5. **Search across time.** Type **`depth >= 5`**. It teleports straight to the deepest point of the recursion — the bottom of the call stack.
6. **Catch the bug before it happens.** Open **`examples/buggy.cvm`**, jump to the end, step back one: you're standing on the exact step *before* a division-by-zero fault.

---

## Track B — Terminal TUI (for the CLI crowd)

```sh
cargo run -- debug examples/recursive.cvm
```

1. **Opens parked at the end** — recursive factorial finished, `result == 120`.
2. Hold **←** (or **[** to leap 25) — the call stack unwinds `fact → fact → …`, each frame with its own `n`. **→** / **]** go forward.
3. Press **space** — it replays forward like a video (**space** again pauses).
4. Press **tab** to select the **`result`** local, then **w** — *"why is `result == 120`?"* chronovm jumps to the cause and lists the chain; **↑ / ↓** walk each hop, moving the whole machine to that moment.
5. Press **/** and type **`depth >= 5`** — teleport to the deepest recursion. **n / N** walk matches.
6. Quit with **q**, then:
   ```sh
   cargo run -- debug examples/buggy.cvm
   ```
   Press **end**, then **←** once — you're one step before the div-by-zero fault. Search **`fault`** lands on the same spot.

### Key map

| Key | Action |
| --- | --- |
| **← / →** | Step one instruction back / forward |
| **[ / ]** | Leap 25 steps |
| **space** | Play / pause (replay like a video) |
| **home / end** | Jump to start / end of the run |
| **tab** | Pick a variable |
| **w** | **Why?** — jump to the cause of the selected value |
| **↑ / ↓** | Walk the causal chain (panel open) |
| **/** | **Search time** — jump to a step matching a condition |
| **n / N** | Next / previous match |
| **q** | Quit |

Search operators: `== != < > <= >=`. Try `acc > 100`, `n == 0`, `depth >= 5`, `top < 0`, `fault`.

---

## Why this is impressive

Most debuggers only step *forward* and let you inspect *what* a value is. chronovm records the entire run so you can scrub time in O(1) **and** answer *why* — walking causality backwards through arithmetic, variables, and recursive call frames to the exact instruction that's to blame.
