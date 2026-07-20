# chronovm architecture

A technical deep-dive into how chronovm works, for engineers. chronovm is a
stack-based bytecode VM whose interpreter *records every step* of an execution
into an immutable trace, so a debugger can scrub the program backwards like a
video and answer causal "why is this value what it is?" questions.

This document explains the recording model, value provenance and the causal
engine, per-call-frame locals, the query language, and the browser bridge. It
tracks the real code in `src/*.rs` — file and symbol references are to the
actual sources.

---

## 1. Module map and the feature split

The crate is deliberately layered so that one VM core drives two very different
UIs (a native terminal debugger and a browser app) without duplicating any of
the interesting logic.

```
                       ┌──────────────────────────────────────┐
   source (.cvm) ───▶  │  assembler.rs   text → Program        │
                       └──────────────────────────────────────┘
                                        │  Program { code, source, labels_at }
                                        ▼
                       ┌──────────────────────────────────────┐
                       │  vm.rs   record(Program) → Trace      │  ← the core
                       │    · Machine (mutable interpreter)    │
                       │    · Frame   (immutable snapshot)     │
                       │    · Trace   (Program + Vec<Frame>)   │
                       │    · explain_var / explain_step (BFS) │
                       └──────────────────────────────────────┘
                                        │  Trace
                    ┌───────────────────┼───────────────────┐
                    ▼                                       ▼
          ┌───────────────────┐                  ┌───────────────────┐
          │  tui.rs  (feature │                  │  wasm.rs (feature │
          │  "tui", native)   │                  │  "wasm", browser) │
          │  ratatui + xterm  │                  │  Session → JSON   │
          └───────────────────┘                  └───────────────────┘
                    │                                       │
              query.rs (shared: parse → Predicate::holds)  ─┘
```

| Module          | Role |
|-----------------|------|
| `isa.rs`        | The instruction set (`Instruction` enum) and the assembled `Program` type. Jump targets are already resolved to absolute instruction indices — the VM never sees a label. |
| `assembler.rs`  | Two-pass assembler: `assemble(&str) -> Result<Program, AsmError>`. Pass one maps labels to instruction indices; pass two parses instructions and resolves label operands. |
| `vm.rs`         | The recording interpreter. `record(Program) -> Trace` runs to completion and snapshots a `Frame` after every instruction. Also home to the causal engine (`explain_var`, `explain_step`). |
| `query.rs`      | The timeline query language: `parse(&str) -> Predicate`, and `Predicate::holds(&Frame) -> bool`. Shared by both UIs. |
| `tui.rs`        | The terminal time-travel debugger (ratatui + crossterm). |
| `wasm.rs`       | wasm-bindgen bridge: a `Session` that serves frames / causal chains / search results to JavaScript as JSON. |

The split is expressed in `lib.rs` and `Cargo.toml`:

```rust
// lib.rs
pub mod assembler;
pub mod isa;
pub mod vm;
pub(crate) mod query;         // internal, used by both UIs

#[cfg(feature = "tui")]  pub mod tui;
#[cfg(feature = "wasm")] mod wasm;
```

```toml
# Cargo.toml
[features]
default = ["tui"]
tui  = ["dep:ratatui", "dep:crossterm"]                       # native only
wasm = ["dep:wasm-bindgen", "dep:serde", "dep:serde-wasm-bindgen"]

[[bin]]
name = "chronovm"
required-features = ["tui"]   # the binary needs the terminal UI
```

The core (`isa`, `assembler`, `vm`, `query`) has no UI dependencies and compiles
everywhere including `wasm32`. The `chronovm` binary (`main.rs`) is gated on the
`tui` feature; the browser build turns `tui` off and `wasm` on. Because the VM
and causal engine live below the feature gates, the exact same recording and
"why?" logic runs in the terminal and in the browser — the UIs are pure views
over an already-computed `Trace`.

`main.rs` is the CLI entry point: `chronovm debug <file>` records and opens the
TUI; `chronovm run <file>` records headless and prints `trace.frames[last].output`.

---

## 2. The recording model: one immutable Frame per step

A conventional interpreter mutates state in place and, once a step is over, the
prior state is gone — to go "back" you must re-run from the start. chronovm
instead **materializes history**. The interpreter (`Machine`, private to `vm.rs`)
is mutable, but callers never touch it. They only ever see the immutable output:

```rust
pub struct Trace {
    pub program: Program,
    pub frames: Vec<Frame>,   // frame 0 = initial state, then one per step
}
```

`record()` builds it by looping the machine and pushing a snapshot after every
instruction:

```rust
pub fn record(program: Program) -> Trace {
    let mut m = Machine::new(&program);
    let mut frames = vec![m.snapshot(None, None, Vec::new(), None, None)]; // frame 0
    while !m.halted {
        // (STEP_LIMIT guard elided — caps runaway loops at 2_000_000 steps)
        let frame = m.step();          // execute exactly one instruction…
        let halted = frame.halted;
        frames.push(frame);            // …and archive the resulting state
        if halted { break; }
    }
    Trace { program, frames }
}
```

A `Frame` is a full snapshot of the machine plus the metadata needed to explain
how it got there:

```rust
pub struct Frame {
    pub ip: usize,                    // next instruction to run
    pub last_op: Option<Instruction>, // the instruction that produced THIS frame
    pub last_ip: Option<usize>,       // its index (for source highlighting)

    pub stack: Vec<i64>,              // the value stack, bottom→top
    pub stack_origin: Vec<usize>,     // provenance, index-aligned with `stack`

    pub call_stack: Vec<Scope>,       // main at [0], current function at top

    pub output: String,               // cumulative program output so far

    pub reads: Vec<usize>,            // steps whose values this op consumed
    pub wrote_var: Option<String>,    // the variable a `store` wrote, if any

    pub error: Option<String>,        // fault message, if this step faulted
    pub halted: bool,
}
```

The consequence is the whole point of the project: **time-travel is `O(1)`
indexing, not replay.** Scrubbing to step *N* is `&trace.frames[N]`. The TUI
states this directly — the entire UI is a pure function of one integer,
`cursor`, and "moving backward is as cheap as moving forward" because both are
just array indexing. `Machine::snapshot` clones the stack, origins, call stack,
and output into each frame, trading memory for the ability to seek anywhere
instantly.

Faults do not discard history. On a recoverable-to-report error (stack
underflow, division by zero, integer overflow) `step()` sets `halted` and
returns a final frame carrying `error: Some(msg)` — every frame up to the fault
is still in the trace, so you can scrub back and watch the machine walk into the
bug. Arithmetic uses `checked_*` operations precisely so overflow becomes a
clean VM fault instead of a panic that would tear down the raw-mode terminal.

---

## 3. Value provenance and the causal engine

This is chronovm's headline feature. Every value on the stack carries the **step
that produced it**, and provenance flows *through variables* on load/store. That
is what lets the debugger answer "why is `x` == 120 here?" — even when the answer
crosses function calls.

### Provenance on the stack

The machine keeps two parallel vectors in lockstep: `stack: Vec<i64>` and
`origin: Vec<usize>`. Every push records an origin step; every pop returns both:

```rust
fn push(&mut self, v: i64, origin: usize) { self.stack.push(v); self.origin.push(origin); }
fn pop(&mut self)  -> Option<(i64, usize)> { Some((self.stack.pop()?, self.origin.pop()?)) }
```

When an instruction *consumes* values, it records their origins in `reads`, and
when it *produces* a value, that value's origin is the current step. For a binary
op:

```rust
fn binary(&mut self, reads: &mut Vec<usize>, this_step: usize,
          f: impl FnOnce(i64, i64) -> Result<i64, String>) -> Option<String> {
    let (b, ob) = self.pop().unwrap();
    let (a, oa) = self.pop().unwrap();
    reads.push(oa); reads.push(ob);      // "I consumed the values from steps oa, ob"
    self.push(f(a, b)?, this_step);      // "the result originates at this step"
    ...
}
```

So each frame ends up with a `reads` list — the immediate causal parents of
whatever this instruction did. Across the whole trace, `reads` edges form a
**provenance DAG** over steps.

### Provenance flowing through variables

The subtle part is `load`/`store`. A `store` remembers which step last wrote the
variable (`locals_def`); a later `load` republishes that step as the loaded
value's origin, so the data dependency survives the round-trip through a named
variable:

```rust
Instruction::Store(name) => {
    let (v, o) = self.pop()?;            // o = origin of the value being stored
    reads.push(o);
    let scope = self.scope();
    scope.locals.insert(name.clone(), v);
    scope.locals_def.insert(name.clone(), this_step);  // remember WHO wrote it
    wrote_var = Some(name.clone());
}

Instruction::Load(name) => {
    let v   = self.scope().locals.get(name).copied()?;
    let def = self.scope().locals_def.get(name).copied().unwrap_or(0);
    reads.push(def);                     // depends on the defining store
    self.push(v, def);                   // loaded value's origin = that store
}
```

Because a store's origin points at the value it consumed, and a load re-emits the
store, a chain like `push 6 → store x → … → load x → mul → store z` stays
connected end to end. The `provenance_flows_through_variables` test in `vm.rs`
asserts exactly this: asking why `z` is what it is reaches the `mul`, and the
`mul` traces back through the loads to where `x` and `y` were set.

### explain_var / explain_step: BFS over the provenance graph

Given a variable, `explain_var` finds the step that defined it in the current
scope, then hands off to `explain_step`, a breadth-first walk backward over
`reads`:

```rust
pub fn explain_var(&self, frame_idx: usize, var: &str) -> Vec<CausalNode> {
    let frame = &self.frames[frame_idx];
    let Some(&def_step) = frame.var_def().get(var) else {
        return vec![/* "`var` was never written before this point" */];
    };
    self.explain_step(def_step, 32)     // budget of 32 nodes
}

pub fn explain_step(&self, start: usize, budget: usize) -> Vec<CausalNode> {
    let mut chain = Vec::new();
    let mut seen  = HashSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(step) = queue.pop_front() {
        if chain.len() >= budget || !seen.insert(step) { continue; } // de-dup + cap
        let f = &self.frames[step];
        chain.push(CausalNode { step, description: self.describe(f) });
        for &r in &f.reads { queue.push_back(r); }   // enqueue causal parents
    }
    chain
}
```

Properties worth noting:

- **Ordered, most-recent-cause-first.** BFS from the defining store outward means
  the chain reads as "this value came from *this* op, which used *these* earlier
  values, …".
- **De-duplicated and bounded.** A `HashSet` guards against re-visiting a step
  that feeds multiple consumers (the graph is a DAG, not a tree), and `budget`
  (32 for variables) caps the chain so a deep computation still returns a
  readable explanation.
- **Crosses call boundaries for free.** Because origins are plain global step
  indices and are passed on the shared value stack, a value computed inside a
  callee and returned to the caller keeps its origin. Walking `reads` therefore
  steps out of and into function calls transparently — the causal chain follows
  the data, not the control flow.

`describe()` renders each node, e.g. `mul  ⇒ 120` or `store total`. In the TUI,
pressing `w` on a selected variable runs `explain_var` and teleports the cursor
to the producing step, showing the chain; `wasm.rs` exposes the same via
`Session::explainVar`.

---

## 4. Per-call-frame locals (Scope): recursion and the reverse call stack

Variables are not global. Each function activation gets its own `Scope`, so a
recursive call's `n` is independent of its caller's `n`:

```rust
pub struct Scope {
    pub func: String,                       // label, or "main" at the top level
    pub return_ip: usize,                   // where to resume in the caller
    pub locals: BTreeMap<String, i64>,      // this frame's variables
    pub locals_def: BTreeMap<String, usize>,// per-local, the step that last wrote it
}
```

The machine holds a `call_stack: Vec<Scope>` that is never empty — it starts with
a single `main` scope (`return_ip: usize::MAX`). All variable reads and writes go
through `self.scope()`, i.e. `call_stack.last_mut()`, so they always hit the
currently executing frame.

`Call` and `Ret` manipulate the call stack while **arguments and return values
travel on the shared value stack** — the callee simply pops what the caller
pushed:

```rust
Instruction::Call { target, name } => {
    self.call_stack.push(Scope {          // fresh locals for the callee
        func: name.clone(),
        return_ip: self.ip + 1,           // resume after the call
        locals: BTreeMap::new(),
        locals_def: BTreeMap::new(),
    });
    self.ip = *target;
}
Instruction::Ret => {
    if self.call_stack.len() > 1 {
        let scope = self.call_stack.pop().unwrap();  // return value stays on the stack
        self.ip = scope.return_ip;
    } else {
        self.halted = true;               // `ret` from main ends the program
    }
}
```

Two payoffs:

1. **Correct recursion.** The `recursion_has_independent_frame_locals` test runs
   a recursive `fact(4)`, checks the result is `24`, and asserts call depth
   reached ≥ 4 — which only works because each frame keeps its own `n`.
2. **A meaningful, unwinding call-stack panel.** Since every `Frame` snapshots the
   whole `call_stack` (bottom = `main`, top = current function), the debugger can
   render the live call stack at *any* point in history, not just the present.

`Frame::current()` / `vars()` / `var_def()` expose the top-of-stack scope, so the
variable panels and causal queries always operate on the locals visible at that
moment in time.

---

## 5. The query language: parse → Predicate::holds

`query.rs` is a small language for searching the timeline: you type a condition
and chronovm jumps to the first step where it holds. Supported forms:

```
acc > 100      variable compared to a number  (== != < > <= >= and = all parse)
n == 0
depth >= 4     `depth` = call-stack depth      (find deep recursion)
top < 0        `top`   = value on top of stack
fault          the step where execution faulted
```

Parsing is a straight split on the first comparison operator (two-char operators
tried first, so `>=` is not mis-read as `>`), yielding a `Predicate`:

```rust
pub enum Predicate {
    Fault,
    Cmp { lhs: Operand, op: CmpOp, rhs: i64 },
}
pub enum Operand { Depth, Top, Var(String) }
```

Evaluation is a pure function of a single frame:

```rust
impl Predicate {
    pub fn holds(&self, frame: &Frame) -> bool {
        match self {
            Predicate::Fault => frame.error.is_some(),
            Predicate::Cmp { lhs, op, rhs } =>
                resolve(lhs, frame).map_or(false, |v| op.apply(v, *rhs)),
        }
    }
}
```

`resolve` maps `depth` → `call_stack.len()`, `top` → `stack.last()`, and a
variable name to its value by scanning the call stack **from the innermost frame
outward** — so a local that only exists inside a function is still findable while
you are in (or below) that call. A search is then a linear scan
`(0..=last).filter(|i| pred.holds(&frames[i]))`. Both UIs share this module: the
TUI's `/`-search and `wasm.rs`'s `Session::search` call the same `parse` +
`holds`.

---

## 6. The wasm bridge: Session serves JSON to the browser

`wasm.rs` is intentionally thin — it is a serialization layer, not logic. The
recorded `Trace` lives in wasm linear memory inside a `Session`, and the browser
pulls plain-JSON views out of it as the user scrubs. No VM logic is
re-implemented for the web; the browser is just another view over the same
`Trace`.

```rust
#[wasm_bindgen]
pub struct Session { trace: Trace }

#[wasm_bindgen]
impl Session {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str) -> Result<Session, JsError> {   // assemble + record once
        let program = assembler::assemble(source).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Session { trace: vm::record(program) })
    }

    #[wasm_bindgen(js_name = frameCount)] pub fn frame_count(&self) -> usize { … }
    pub fn program(&self)               -> Result<JsValue, JsError> { … } // listing
    pub fn frame(&self, i: usize)       -> Result<JsValue, JsError> { … } // full state @ i
    #[wasm_bindgen(js_name = explainVar)]
    pub fn explain_var(&self, i: usize, var: &str) -> Result<JsValue, JsError> { … } // causal chain
    pub fn search(&self, q: &str)       -> Result<JsValue, JsError> { … } // query::parse + holds
}
```

Each endpoint maps the internal types to `#[derive(Serialize)]` view structs
(`FrameView`, `StackSlot { value, origin }`, `ScopeView`, `CausalNodeView`,
`SearchResult { matches }`) via `serde_wasm_bindgen`. Note that provenance
survives the boundary — every `StackSlot` and `VarView` carries its `origin` /
`def` step, so the web UI can offer the same "why?" navigation as the terminal.
The `call_stack` is serialized bottom-to-top, matching the terminal panel order,
so both UIs render the stack identically.

The lifecycle is: construct a `Session` once from source (assembling + recording
eagerly), then treat it as an immutable database — `frame(i)`, `explainVar(i,
var)`, and `search(q)` are all cheap, side-effect-free lookups into the frame
vector. This mirrors the TUI exactly, where the UI is a pure function of the
`cursor` index. The two front ends differ only in rendering; the model beneath
them is one and the same recorded `Trace`.

---

## Recap of the key ideas

- **Record, don't replay.** One immutable `Frame` per instruction makes seeking
  to any point in time `O(1)` array indexing.
- **Values remember where they came from.** Parallel `stack`/`origin` vectors,
  plus provenance that flows through variables via `locals_def`, build a DAG of
  step-to-step data dependencies.
- **"Why?" is a graph walk.** `explain_var`/`explain_step` BFS that DAG backward,
  bounded and de-duplicated, and it crosses function calls because origins are
  global step indices carried on the shared stack.
- **Per-frame `Scope`s** give correct recursion and a call stack you can inspect
  at any historical moment.
- **One core, two skins.** The feature split keeps the VM, causal engine, and
  query language UI-agnostic, so the ratatui terminal and the wasm browser app
  are thin, interchangeable views over the same `Trace`.
