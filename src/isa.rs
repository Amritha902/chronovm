//! The instruction set for the chronovm stack machine.
//!
//! Programs are flat lists of [`Instruction`]s. Jump targets are resolved to
//! absolute instruction indices by the assembler, so the VM never deals with
//! labels — it just moves an instruction pointer around.

use std::fmt;

/// A single VM instruction. The machine is a classic stack machine with a set
/// of named variables (locals) on the side.
#[derive(Clone, Debug, PartialEq)]
pub enum Instruction {
    /// Push an integer literal onto the stack.
    Push(i64),
    /// Discard the top of the stack.
    Pop,
    /// Duplicate the top of the stack.
    Dup,
    /// Swap the top two stack values.
    Swap,

    // --- arithmetic (pop b, pop a, push a `op` b) ---
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    /// Negate the top of the stack.
    Neg,

    // --- comparison (push 1 for true, 0 for false) ---
    Eq,
    Lt,
    Gt,
    Le,
    Ge,
    /// Logical not: pop x, push 1 if x == 0 else 0.
    Not,

    /// Push the current value of a named variable.
    Load(String),
    /// Pop the top of the stack into a named variable.
    Store(String),

    /// Unconditional jump to an instruction index.
    Jmp(usize),
    /// Pop x; jump if x == 0.
    Jz(usize),
    /// Pop x; jump if x != 0.
    Jnz(usize),

    /// Call a function: push a new call frame (with its own locals) and jump to
    /// `target`. `name` is the label, kept for the debugger's call-stack panel.
    /// Arguments and return values are passed on the shared value stack.
    Call {
        target: usize,
        name: String,
    },
    /// Return from the current function: pop the call frame and resume in the
    /// caller. Any value left on the stack is the return value.
    Ret,

    /// Pop x and append it to the program output.
    Print,
    /// Stop execution.
    Halt,
}

impl Instruction {
    /// A short human-readable mnemonic, used in the debugger's source pane.
    pub fn mnemonic(&self) -> String {
        match self {
            Instruction::Push(n) => format!("push {n}"),
            Instruction::Pop => "pop".into(),
            Instruction::Dup => "dup".into(),
            Instruction::Swap => "swap".into(),
            Instruction::Add => "add".into(),
            Instruction::Sub => "sub".into(),
            Instruction::Mul => "mul".into(),
            Instruction::Div => "div".into(),
            Instruction::Mod => "mod".into(),
            Instruction::Neg => "neg".into(),
            Instruction::Eq => "eq".into(),
            Instruction::Lt => "lt".into(),
            Instruction::Gt => "gt".into(),
            Instruction::Le => "le".into(),
            Instruction::Ge => "ge".into(),
            Instruction::Not => "not".into(),
            Instruction::Load(v) => format!("load {v}"),
            Instruction::Store(v) => format!("store {v}"),
            Instruction::Jmp(t) => format!("jmp @{t}"),
            Instruction::Jz(t) => format!("jz @{t}"),
            Instruction::Jnz(t) => format!("jnz @{t}"),
            Instruction::Call { name, .. } => format!("call {name}"),
            Instruction::Ret => "ret".into(),
            Instruction::Print => "print".into(),
            Instruction::Halt => "halt".into(),
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.mnemonic())
    }
}

/// A fully assembled program: instructions plus the original source line each
/// instruction came from (so the debugger can show the human-written source).
#[derive(Clone, Debug)]
pub struct Program {
    pub code: Vec<Instruction>,
    /// The verbatim source text for each instruction, index-aligned with `code`.
    pub source: Vec<String>,
    /// Reverse map from instruction index to any labels pointing at it.
    pub labels_at: Vec<Vec<String>>,
}

impl Program {
    pub fn len(&self) -> usize {
        self.code.len()
    }

    pub fn is_empty(&self) -> bool {
        self.code.is_empty()
    }
}
