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

    /// The value stack, bottom-to-top.
    pub stack: Vec<i64>,
    /// Provenance, index-aligned with `stack`: the step that produced each value.
    pub stack_origin: Vec<usize>,

    /// Named variables and their current values.
    pub vars: BTreeMap<String, i64>,
    /// For each variable, the step whose `store` last wrote it.
    pub var_def: BTreeMap<String, usize>,

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
        self.frames
            .last()
            .and_then(|f| f.error.as_deref())
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
        let frame = &self.frames[frame_idx];
        let Some(&def_step) = frame.var_def.get(var) else {
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
            let f = &self.frames[step];
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
                Some(format!("step limit ({STEP_LIMIT}) exceeded — infinite loop?")),
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
    vars: BTreeMap<String, i64>,
    var_def: BTreeMap<String, usize>,
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
            vars: BTreeMap::new(),
            var_def: BTreeMap::new(),
            output: String::new(),
            halted: false,
            last_op: None,
            prev_ip: 0,
        }
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
            vars: self.vars.clone(),
            var_def: self.var_def.clone(),
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

            Instruction::Add => self.binary(&mut reads, this_step, |a, b| Ok(a + b)),
            Instruction::Sub => self.binary(&mut reads, this_step, |a, b| Ok(a - b)),
            Instruction::Mul => self.binary(&mut reads, this_step, |a, b| Ok(a * b)),
            Instruction::Div => self.binary(&mut reads, this_step, |a, b| {
                if b == 0 {
                    Err("division by zero".into())
                } else {
                    Ok(a / b)
                }
            }),
            Instruction::Mod => self.binary(&mut reads, this_step, |a, b| {
                if b == 0 {
                    Err("modulo by zero".into())
                } else {
                    Ok(a % b)
                }
            }),
            Instruction::Neg => match self.pop() {
                Some((a, o)) => {
                    reads.push(o);
                    self.push(-a, this_step);
                    self.ip += 1;
                    None
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

            Instruction::Load(name) => match self.vars.get(name).copied() {
                Some(v) => {
                    // Provenance flows through the variable: the loaded value's
                    // origin is the step that last stored it.
                    let def = self.var_def.get(name).copied().unwrap_or(0);
                    reads.push(def);
                    self.push(v, def);
                    self.ip += 1;
                    None
                }
                None => Some(format!("load of undefined variable `{name}`")),
            },
            Instruction::Store(name) => match self.pop() {
                Some((v, o)) => {
                    reads.push(o);
                    self.vars.insert(name.clone(), v);
                    self.var_def.insert(name.clone(), this_step);
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
        assert_eq!(last.vars.get("z"), Some(&42));
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
}
