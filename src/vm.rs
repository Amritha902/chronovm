//! The recording virtual machine.
//!
//! Unlike a normal interpreter, this VM does not just compute a result — it
//! records a full [`Frame`] after every single instruction. The complete
//! [`Trace`] is what makes time-travel possible: scrubbing to step N is just an
//! index into a vector, so moving backwards is O(1) no matter how long the
//! program ran.
//!
//! Every value also carries *provenance*: the step that produced it. When a
//! value is stored into a variable and later loaded, the provenance flows
//! through the variable. That lets us answer "why is `x` == 120 here?" by
//! walking the chain of producing steps backwards — the debugger's headline
//! feature.

use std::collections::BTreeMap;

use crate::isa::{Instruction, Program};

/// The largest number of steps we will execute before assuming a program has
/// run away. Keeps a buggy `jmp` loop from eating all memory during recording.
pub const STEP_LIMIT: usize = 2_000_000;

/// Upper bound on addressable scratch memory cells (`mstore`/`mload`), to keep
/// a bad address from triggering a huge allocation.
pub const MAX_MEM: usize = 1 << 16;

/// One activation record on the call stack. Each function call gets its own
/// `Scope`, so a variable named `n` in a recursive call is independent of the
/// caller's `n`. This is what makes the debugger's unwinding call-stack panel
/// meaningful.
#[derive(Clone, Debug)]
pub struct Scope {
    /// The function's name (its label), or "main" for the top level.
    pub func: String,
    /// The instruction index to resume at in the caller once this frame returns.
    pub return_ip: usize,
    /// This frame's local variables.
    pub locals: BTreeMap<String, i64>,
    /// For each local, the step whose `store` last wrote it.
    pub locals_def: BTreeMap<String, usize>,
}

/// A snapshot of the entire machine at one point in time, plus the metadata
/// needed to explain *how* it got there.
#[derive(Clone, Debug)]
pub struct Frame {
    /// Instruction pointer: the index of the instruction about to execute next.
    pub ip: usize,
    /// The instruction that was executed to *produce* this frame (None for the
    /// initial frame).
    pub last_op: Option<Instruction>,
    /// The instruction index of `last_op`, so the UI can highlight the source
    /// line that just ran.
    pub last_ip: Option<usize>,

    /// The value stack, bottom-to-top. Shared across all call frames — this is
    /// how arguments and return values are passed.
    pub stack: Vec<i64>,
    /// Provenance, index-aligned with `stack`: the step that produced each value.
    pub stack_origin: Vec<usize>,

    /// The call stack, bottom (main) to top (currently executing function).
    /// Always contains at least the `main` scope.
    pub call_stack: Vec<Scope>,

    /// Linear memory contents up to the current high-water mark (empty until a
    /// program uses `mstore`). Lets the UI show an array mutating over time.
    pub memory: Vec<i64>,

    /// Cumulative program output up to and including this step.
    pub output: String,

    /// The steps whose produced values this instruction consumed. This is the
    /// backbone of causal queries.
    pub reads: Vec<usize>,
    /// The variable this step wrote, if it was a `store`.
    pub wrote_var: Option<String>,

    /// If execution faulted at this step, the error message.
    pub error: Option<String>,
    /// True once the machine has halted (cleanly or via fault).
    pub halted: bool,
}

impl Frame {
    /// The currently executing scope (top of the call stack).
    pub fn current(&self) -> &Scope {
        self.call_stack.last().expect("call stack is never empty")
    }

    /// The locals visible at this point in time (the current scope's).
    pub fn vars(&self) -> &BTreeMap<String, i64> {
        &self.current().locals
    }

    /// The defining step for each visible local.
    pub fn var_def(&self) -> &BTreeMap<String, usize> {
        &self.current().locals_def
    }
}

/// A complete recorded execution: the program plus every frame it produced.
pub struct Trace {
    pub program: Program,
    pub frames: Vec<Frame>,
}

impl Trace {
    /// The last frame index (there is always at least the initial frame).
    pub fn last(&self) -> usize {
        self.frames.len() - 1
    }

    /// Did the program end in an error?
    pub fn faulted(&self) -> Option<&str> {
        self.frames.last().and_then(|f| f.error.as_deref())
    }
}

/// One link in a causal chain: a step that contributed to a value, with a
/// human-readable description of what it did.
#[derive(Clone, Debug)]
pub struct CausalNode {
    pub step: usize,
    pub description: String,
}

impl Trace {
    /// Explain why variable `var` holds its value as of `frame_idx`, by walking
    /// the provenance graph backwards from the `store` that defined it.
    ///
    /// Returns an ordered chain, most-recent cause first. This is the engine
    /// behind the debugger's "why is this value what it is?" jump.
    pub fn explain_var(&self, frame_idx: usize, var: &str) -> Vec<CausalNode> {
        // Bounds-check: the browser can pass any frame index, and an out-of-range
        // index would otherwise panic (a wasm trap) rather than return cleanly.
        let Some(frame) = self.frames.get(frame_idx) else {
            return Vec::new();
        };
        let Some(&def_step) = frame.var_def().get(var) else {
            return vec![CausalNode {
                step: frame_idx,
                description: format!("`{var}` was never written before this point"),
            }];
        };
        self.explain_step(def_step, 32)
    }

    /// Walk the provenance DAG backwards from `start` in breadth-first order,
    /// producing an ordered, de-duplicated causal chain of at most `budget`
    /// nodes.
    pub fn explain_step(&self, start: usize, budget: usize) -> Vec<CausalNode> {
        let mut chain = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start);

        while let Some(step) = queue.pop_front() {
            if chain.len() >= budget || !seen.insert(step) {
                continue;
            }
            let Some(f) = self.frames.get(step) else {
                continue;
            };
            chain.push(CausalNode {
                step,
                description: self.describe(f),
            });
            for &r in &f.reads {
                queue.push_back(r);
            }
        }
        chain
    }

    /// A one-line description of what a frame's instruction did, e.g.
    /// "mul → 120" or "store total".
    fn describe(&self, f: &Frame) -> String {
        let op = f
            .last_op
            .as_ref()
            .map(|o| o.mnemonic())
            .unwrap_or_else(|| "start".into());
        match f.stack.last() {
            Some(top) if f.wrote_var.is_none() => format!("{op}  ⇒ {top}"),
            _ => op,
        }
    }
}

/// Run a program to completion (or fault, or step limit), recording every step.
pub fn record(program: Program) -> Trace {
    let mut m = Machine::new(&program);
    let mut frames = vec![m.snapshot(None, None, Vec::new(), None, None)];

    while !m.halted {
        if frames.len() > STEP_LIMIT {
            let mut last = m.snapshot(
                m.last_op.clone(),
                Some(m.prev_ip),
                Vec::new(),
                None,
                Some(format!(
                    "step limit ({STEP_LIMIT}) exceeded — infinite loop?"
                )),
            );
            last.halted = true;
            frames.push(last);
            break;
        }
        let frame = m.step();
        let halted = frame.halted;
        frames.push(frame);
        if halted {
            break;
        }
    }

    Trace { program, frames }
}

/// The mutable execution state. Not exposed outside this module — callers only
/// ever see the immutable [`Trace`] it produces.
struct Machine<'p> {
    code: &'p [Instruction],
    ip: usize,
    step: usize,
    stack: Vec<i64>,
    origin: Vec<usize>,
    /// Linear scratch memory addressed by `mstore`/`mload`, grown on demand.
    mem: Vec<i64>,
    /// Provenance for each memory cell (the step that last wrote it), so causal
    /// queries flow through memory just like they flow through variables.
    mem_origin: Vec<usize>,
    /// The call stack; always non-empty (the base scope is `main`).
    call_stack: Vec<Scope>,
    output: String,
    halted: bool,
    // Bookkeeping so a step-limit frame can report the last op/ip it saw.
    last_op: Option<Instruction>,
    prev_ip: usize,
}

impl<'p> Machine<'p> {
    fn new(program: &'p Program) -> Self {
        Machine {
            code: &program.code,
            ip: 0,
            step: 0,
            stack: Vec::new(),
            origin: Vec::new(),
            mem: Vec::new(),
            mem_origin: Vec::new(),
            call_stack: vec![Scope {
                func: "main".into(),
                return_ip: usize::MAX,
                locals: BTreeMap::new(),
                locals_def: BTreeMap::new(),
            }],
            output: String::new(),
            halted: false,
            last_op: None,
            prev_ip: 0,
        }
    }

    /// Mutable access to the currently executing scope's locals.
    fn scope(&mut self) -> &mut Scope {
        self.call_stack.last_mut().expect("call stack never empty")
    }

    /// Capture the current state as an immutable frame.
    fn snapshot(
        &self,
        last_op: Option<Instruction>,
        last_ip: Option<usize>,
        reads: Vec<usize>,
        wrote_var: Option<String>,
        error: Option<String>,
    ) -> Frame {
        Frame {
            ip: self.ip,
            last_op,
            last_ip,
            stack: self.stack.clone(),
            stack_origin: self.origin.clone(),
            call_stack: self.call_stack.clone(),
            memory: self.mem.clone(),
            output: self.output.clone(),
            reads,
            wrote_var,
            error,
            halted: self.halted,
        }
    }

    /// Execute exactly one instruction and return the resulting frame.
    fn step(&mut self) -> Frame {
        // Out of bounds instruction pointer ⇒ implicit clean halt.
        if self.ip >= self.code.len() {
            self.halted = true;
            self.step += 1;
            return self.snapshot(None, None, Vec::new(), None, None);
        }

        let cur_ip = self.ip;
        let instr = self.code[cur_ip].clone();
        self.prev_ip = cur_ip;
        self.last_op = Some(instr.clone());
        self.step += 1;

        // `reads` accumulates the origin steps of every value this instruction
        // consumes, so causal queries can follow the data backwards.
        let mut reads: Vec<usize> = Vec::new();
        let mut wrote_var: Option<String> = None;
        let this_step = self.step;

        // Execute. On a recoverable-to-report error we set `err` and halt.
        let err: Option<String> = match &instr {
            Instruction::Push(n) => {
                self.push(*n, this_step);
                self.ip += 1;
                None
            }
            Instruction::Pop => match self.pop() {
                Some((_v, o)) => {
                    reads.push(o);
                    self.ip += 1;
                    None
                }
                None => Some(underflow("pop")),
            },
            Instruction::Dup => match self.peek() {
                Some((v, o)) => {
                    reads.push(o);
                    self.push(v, this_step);
                    self.ip += 1;
                    None
                }
                None => Some(underflow("dup")),
            },
            Instruction::Swap => {
                if self.stack.len() < 2 {
                    Some(underflow("swap"))
                } else {
                    let n = self.stack.len();
                    self.stack.swap(n - 1, n - 2);
                    self.origin.swap(n - 1, n - 2);
                    // Both operands are "touched"; record their (post-swap) origins.
                    reads.push(self.origin[n - 1]);
                    reads.push(self.origin[n - 2]);
                    self.ip += 1;
                    None
                }
            }

            // Arithmetic uses checked operations so overflow becomes a clean VM
            // fault instead of a panic that would tear down the raw-mode TUI.
            Instruction::Add => self.binary(&mut reads, this_step, |a, b| {
                a.checked_add(b)
                    .ok_or_else(|| "integer overflow in add".to_string())
            }),
            Instruction::Sub => self.binary(&mut reads, this_step, |a, b| {
                a.checked_sub(b)
                    .ok_or_else(|| "integer overflow in sub".to_string())
            }),
            Instruction::Mul => self.binary(&mut reads, this_step, |a, b| {
                a.checked_mul(b)
                    .ok_or_else(|| "integer overflow in mul".to_string())
            }),
            Instruction::Div => self.binary(&mut reads, this_step, |a, b| {
                if b == 0 {
                    Err("division by zero".into())
                } else {
                    // Catches i64::MIN / -1, which also overflows.
                    a.checked_div(b)
                        .ok_or_else(|| "integer overflow in div".to_string())
                }
            }),
            Instruction::Mod => self.binary(&mut reads, this_step, |a, b| {
                if b == 0 {
                    Err("modulo by zero".into())
                } else {
                    a.checked_rem(b)
                        .ok_or_else(|| "integer overflow in mod".to_string())
                }
            }),
            Instruction::Neg => match self.pop() {
                Some((a, o)) => {
                    reads.push(o);
                    match a.checked_neg() {
                        Some(v) => {
                            self.push(v, this_step);
                            self.ip += 1;
                            None
                        }
                        None => Some("integer overflow in neg".into()),
                    }
                }
                None => Some(underflow("neg")),
            },

            Instruction::Eq => self.binary(&mut reads, this_step, |a, b| Ok((a == b) as i64)),
            Instruction::Lt => self.binary(&mut reads, this_step, |a, b| Ok((a < b) as i64)),
            Instruction::Gt => self.binary(&mut reads, this_step, |a, b| Ok((a > b) as i64)),
            Instruction::Le => self.binary(&mut reads, this_step, |a, b| Ok((a <= b) as i64)),
            Instruction::Ge => self.binary(&mut reads, this_step, |a, b| Ok((a >= b) as i64)),
            Instruction::Not => match self.pop() {
                Some((a, o)) => {
                    reads.push(o);
                    self.push((a == 0) as i64, this_step);
                    self.ip += 1;
                    None
                }
                None => Some(underflow("not")),
            },

            Instruction::Load(name) => {
                // Locals are per call frame, so this reads the current scope.
                match self.scope().locals.get(name).copied() {
                    Some(v) => {
                        // Provenance flows through the variable: the loaded
                        // value's origin is the step that last stored it.
                        let def = self.scope().locals_def.get(name).copied().unwrap_or(0);
                        reads.push(def);
                        self.push(v, def);
                        self.ip += 1;
                        None
                    }
                    None => Some(format!("load of undefined variable `{name}`")),
                }
            }
            Instruction::Store(name) => match self.pop() {
                Some((v, o)) => {
                    reads.push(o);
                    let scope = self.scope();
                    scope.locals.insert(name.clone(), v);
                    scope.locals_def.insert(name.clone(), this_step);
                    wrote_var = Some(name.clone());
                    self.ip += 1;
                    None
                }
                None => Some(underflow("store")),
            },

            Instruction::Jmp(t) => {
                self.ip = *t;
                None
            }
            Instruction::Jz(t) => match self.pop() {
                Some((v, o)) => {
                    reads.push(o);
                    self.ip = if v == 0 { *t } else { self.ip + 1 };
                    None
                }
                None => Some(underflow("jz")),
            },
            Instruction::Jnz(t) => match self.pop() {
                Some((v, o)) => {
                    reads.push(o);
                    self.ip = if v != 0 { *t } else { self.ip + 1 };
                    None
                }
                None => Some(underflow("jnz")),
            },

            Instruction::Call { target, name } => {
                // Push a fresh activation record; arguments stay on the shared
                // value stack for the callee to consume.
                self.call_stack.push(Scope {
                    func: name.clone(),
                    return_ip: self.ip + 1,
                    locals: BTreeMap::new(),
                    locals_def: BTreeMap::new(),
                });
                self.ip = *target;
                None
            }
            Instruction::Ret => {
                if self.call_stack.len() > 1 {
                    // Return value (if any) is left on the shared stack.
                    let scope = self.call_stack.pop().unwrap();
                    self.ip = scope.return_ip;
                    None
                } else {
                    // `ret` from main ends the program cleanly.
                    self.halted = true;
                    None
                }
            }

            Instruction::MStore => {
                // ( value addr -- ) : mem[addr] = value
                if self.stack.len() < 2 {
                    Some(underflow("mstore"))
                } else {
                    let (addr, o_addr) = self.pop().unwrap();
                    let (val, o_val) = self.pop().unwrap();
                    reads.push(o_val);
                    reads.push(o_addr);
                    match self.mem_index(addr) {
                        Ok(i) => {
                            self.mem[i] = val;
                            self.mem_origin[i] = this_step;
                            self.ip += 1;
                            None
                        }
                        Err(e) => Some(e),
                    }
                }
            }
            Instruction::MLoad => match self.pop() {
                // ( addr -- value ) ; provenance flows from the cell's writer
                Some((addr, o_addr)) => match self.mem_index(addr) {
                    Ok(i) => {
                        let origin = self.mem_origin[i];
                        reads.push(origin);
                        reads.push(o_addr);
                        self.push(self.mem[i], origin);
                        self.ip += 1;
                        None
                    }
                    Err(e) => Some(e),
                },
                None => Some(underflow("mload")),
            },

            Instruction::Print => match self.pop() {
                Some((v, o)) => {
                    reads.push(o);
                    self.output.push_str(&v.to_string());
                    self.output.push('\n');
                    self.ip += 1;
                    None
                }
                None => Some(underflow("print")),
            },
            Instruction::Halt => {
                self.halted = true;
                None
            }
        };

        if let Some(message) = err {
            self.halted = true;
            return self.snapshot(Some(instr), Some(cur_ip), reads, wrote_var, Some(message));
        }
        self.snapshot(Some(instr), Some(cur_ip), reads, wrote_var, None)
    }

    // --- small stack helpers that keep value and provenance in lockstep ---

    fn push(&mut self, v: i64, origin: usize) {
        self.stack.push(v);
        self.origin.push(origin);
    }

    /// Validate a memory address and grow the backing store to cover it,
    /// returning the usable index. Faults on a negative or out-of-range address.
    fn mem_index(&mut self, addr: i64) -> Result<usize, String> {
        // Compare in i64 space: on wasm32 `addr as usize` truncates to 32 bits,
        // so a huge address would wrap and slip past a usize-based bound check.
        if addr < 0 || addr >= MAX_MEM as i64 {
            return Err(format!("memory address {addr} out of range (0..{MAX_MEM})"));
        }
        let i = addr as usize;
        if i >= self.mem.len() {
            self.mem.resize(i + 1, 0);
            self.mem_origin.resize(i + 1, 0);
        }
        Ok(i)
    }

    fn pop(&mut self) -> Option<(i64, usize)> {
        Some((self.stack.pop()?, self.origin.pop()?))
    }

    fn peek(&self) -> Option<(i64, usize)> {
        Some((*self.stack.last()?, *self.origin.last()?))
    }

    /// Pop b, pop a, push `f(a, b)`; record both origins.
    fn binary(
        &mut self,
        reads: &mut Vec<usize>,
        this_step: usize,
        f: impl FnOnce(i64, i64) -> Result<i64, String>,
    ) -> Option<String> {
        if self.stack.len() < 2 {
            return Some(underflow("binary op"));
        }
        let (b, ob) = self.pop().unwrap();
        let (a, oa) = self.pop().unwrap();
        reads.push(oa);
        reads.push(ob);
        match f(a, b) {
            Ok(v) => {
                self.push(v, this_step);
                self.ip += 1;
                None
            }
            Err(e) => Some(e),
        }
    }
}

fn underflow(op: &str) -> String {
    format!("stack underflow in `{op}`")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembler::assemble;

    fn trace_of(src: &str) -> Trace {
        record(assemble(src).expect("assembles"))
    }

    #[test]
    fn computes_and_records_every_step() {
        let t = trace_of("push 6\nstore x\npush 7\nstore y\nload x\nload y\nmul\nstore z\nhalt\n");
        let last = &t.frames[t.last()];
        assert_eq!(last.vars().get("z"), Some(&42));
        assert!(last.error.is_none());
        // Frame 0 is the initial state, then one frame per executed instruction.
        assert!(t.frames.len() > 1);
    }

    #[test]
    fn provenance_flows_through_variables() {
        // z = x * y. Asking why z is what it is must surface the `mul` step,
        // and that mul must trace back (via loads) to where x and y were set.
        let t = trace_of("push 6\nstore x\npush 7\nstore y\nload x\nload y\nmul\nstore z\nhalt\n");
        let chain = t.explain_var(t.last(), "z");

        // The chain begins at the `store z` that defined the value.
        let head = &t.frames[chain[0].step];
        assert_eq!(head.wrote_var.as_deref(), Some("z"));

        // Somewhere upstream is the multiplication that produced 42.
        let has_mul = chain
            .iter()
            .any(|n| matches!(t.frames[n.step].last_op, Some(Instruction::Mul)));
        assert!(has_mul, "causal chain should reach the mul: {chain:?}");
    }

    #[test]
    fn division_by_zero_faults_but_keeps_history() {
        let t = trace_of("push 10\npush 0\ndiv\nhalt\n");
        assert_eq!(t.faulted(), Some("division by zero"));
        // The fault does not discard the recorded history leading up to it.
        assert!(t.frames.len() >= 3);
    }

    #[test]
    fn runaway_loop_is_capped() {
        // `jmp self` with no exit must stop at the step limit, not hang.
        let t = trace_of("loop:\njmp loop\n");
        assert!(t.faulted().unwrap().contains("step limit"));
    }

    #[test]
    fn integer_overflow_faults_instead_of_panicking() {
        // i64::MAX + 1 must be reported as a fault, not crash the interpreter.
        let t = trace_of("push 9223372036854775807\npush 1\nadd\nhalt\n");
        assert_eq!(t.faulted(), Some("integer overflow in add"));
        // History up to the fault is intact.
        assert!(t.frames.len() >= 3);
    }

    #[test]
    fn memory_store_load_roundtrips() {
        // mem[3] = 42, then load it back and print.
        let t = trace_of("push 42\npush 3\nmstore\npush 3\nmload\nprint\nhalt\n");
        assert_eq!(t.faulted(), None);
        assert_eq!(t.frames[t.last()].output.trim(), "42");
    }

    #[test]
    fn provenance_flows_through_memory() {
        // x is loaded from a memory cell; "why x?" must reach the mstore.
        let t = trace_of("push 7\npush 3\nmstore\npush 3\nmload\nstore x\nhalt\n");
        let chain = t.explain_var(t.last(), "x");
        let reached_mstore = chain
            .iter()
            .any(|n| matches!(t.frames[n.step].last_op, Some(Instruction::MStore)));
        assert!(
            reached_mstore,
            "causal chain should thread through memory: {chain:?}"
        );
    }

    #[test]
    fn huge_memory_address_cannot_wrap_past_the_bound() {
        // 2^32 truncates to 0 in 32-bit usize; the check must compare in i64
        // space so this faults instead of silently writing to mem[0].
        let t = trace_of("push 7\npush 4294967296\nmstore\nhalt\n");
        assert!(t.faulted().unwrap().contains("out of range"));
    }

    #[test]
    fn explain_var_out_of_range_frame_is_empty_not_a_panic() {
        let t = trace_of("push 1\nstore x\nhalt\n");
        assert!(t.explain_var(usize::MAX, "x").is_empty());
    }

    #[test]
    fn bad_memory_address_faults() {
        let t = trace_of("push 1\npush -5\nmstore\nhalt\n");
        assert!(t.faulted().unwrap().contains("out of range"));
    }

    #[test]
    fn recursion_has_independent_frame_locals() {
        // Recursive factorial: each call frame must keep its own `n`, otherwise
        // the recursion would clobber the caller's value and give a wrong answer.
        let src = "\
            push 4\n\
            call fact\n\
            print\n\
            halt\n\
        fact:\n\
            store n\n\
            load n\n\
            push 1\n\
            le\n\
            jz recurse\n\
            push 1\n\
            ret\n\
        recurse:\n\
            load n\n\
            push 1\n\
            sub\n\
            call fact\n\
            load n\n\
            mul\n\
            ret\n";
        let t = trace_of(src);
        assert_eq!(t.faulted(), None);
        // 4! == 24
        assert_eq!(t.frames[t.last()].output.trim(), "24");
        // The call stack must have grown past depth 1 at some point.
        let max_depth = t.frames.iter().map(|f| f.call_stack.len()).max().unwrap();
        assert!(max_depth >= 4, "expected deep recursion, got {max_depth}");
    }
}
