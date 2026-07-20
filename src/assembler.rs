//! A tiny two-pass assembler for `.cvm` source files.
//!
//! Syntax is deliberately minimal and readable:
//!
//! ```text
//!   ; comments start with ; or #
//!   push 5          ; an instruction with an argument
//!   store n
//! loop:             ; a label (its own line or inline before an instruction)
//!   load n
//!   jz  done        ; jumps refer to labels, resolved to indices in pass two
//!   jmp loop
//! done:
//!   halt
//! ```
//!
//! Pass one records the instruction index of every label. Pass two parses each
//! instruction, resolving label operands to absolute indices.

use crate::isa::{Instruction, Program};

/// An assembly error with the 1-based source line number for good diagnostics.
#[derive(Debug)]
pub struct AsmError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for AsmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for AsmError {}

/// Strip a comment (`;` or `#`) and surrounding whitespace from a line.
fn strip(line: &str) -> &str {
    let end = line.find([';', '#']).unwrap_or(line.len());
    line[..end].trim()
}

/// Assemble source text into a runnable [`Program`].
pub fn assemble(src: &str) -> Result<Program, AsmError> {
    // A "cell" is a source line that carries an instruction. We first split
    // labels from instructions so both passes agree on indices.
    struct Cell {
        line_no: usize,
        text: String,
        labels: Vec<String>,
    }

    let mut cells: Vec<Cell> = Vec::new();
    let mut pending_labels: Vec<String> = Vec::new();

    for (i, raw) in src.lines().enumerate() {
        let line_no = i + 1;
        let mut content = strip(raw);
        if content.is_empty() {
            continue;
        }

        // A line may be `label:` alone, or `label: instr`, possibly repeated.
        while let Some(colon) = content.find(':') {
            let label = content[..colon].trim();
            if label.is_empty() || label.contains(char::is_whitespace) {
                break; // not actually a label prefix (e.g. a stray colon)
            }
            pending_labels.push(label.to_string());
            content = content[colon + 1..].trim();
        }

        if content.is_empty() {
            continue; // label-only line; labels attach to the next instruction
        }

        cells.push(Cell {
            line_no,
            text: content.to_string(),
            labels: std::mem::take(&mut pending_labels),
        });
    }

    // Any labels left over point one past the end (a valid jump target meaning
    // "halt"). We handle that by mapping them to code.len() below.
    let trailing_labels = pending_labels;

    // --- pass one: label -> instruction index ---
    let mut label_index: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (idx, cell) in cells.iter().enumerate() {
        for l in &cell.labels {
            if label_index.insert(l.clone(), idx).is_some() {
                return Err(AsmError {
                    line: cell.line_no,
                    message: format!("duplicate label `{l}`"),
                });
            }
        }
    }
    for l in &trailing_labels {
        label_index.insert(l.clone(), cells.len());
    }

    // --- pass two: parse instructions, resolving label operands ---
    let mut code = Vec::with_capacity(cells.len());
    let mut source = Vec::with_capacity(cells.len());
    let mut labels_at = Vec::with_capacity(cells.len());

    for cell in &cells {
        let instr = parse_instruction(&cell.text, &label_index, cell.line_no)?;
        code.push(instr);
        source.push(cell.text.clone());
        labels_at.push(cell.labels.clone());
    }

    if code.is_empty() {
        return Err(AsmError {
            line: 0,
            message: "program is empty".into(),
        });
    }

    Ok(Program {
        code,
        source,
        labels_at,
    })
}

fn parse_instruction(
    text: &str,
    labels: &std::collections::HashMap<String, usize>,
    line: usize,
) -> Result<Instruction, AsmError> {
    let mut parts = text.split_whitespace();
    let op = parts.next().unwrap().to_lowercase();
    let arg = parts.next();
    let err = |m: String| AsmError { line, message: m };

    // Helpers for the two operand kinds.
    let int_arg = |a: Option<&str>| -> Result<i64, AsmError> {
        a.ok_or_else(|| err(format!("`{op}` expects an integer argument")))?
            .parse::<i64>()
            .map_err(|_| err(format!("`{op}` argument must be an integer")))
    };
    let name_arg = |a: Option<&str>| -> Result<String, AsmError> {
        Ok(
            a.ok_or_else(|| err(format!("`{op}` expects a variable name")))?
                .to_string(),
        )
    };
    let target_arg = |a: Option<&str>| -> Result<usize, AsmError> {
        let name = a.ok_or_else(|| err(format!("`{op}` expects a label")))?;
        labels
            .get(name)
            .copied()
            .ok_or_else(|| err(format!("unknown label `{name}`")))
    };

    let instr = match op.as_str() {
        "push" => Instruction::Push(int_arg(arg)?),
        "pop" => Instruction::Pop,
        "dup" => Instruction::Dup,
        "swap" => Instruction::Swap,
        "add" => Instruction::Add,
        "sub" => Instruction::Sub,
        "mul" => Instruction::Mul,
        "div" => Instruction::Div,
        "mod" => Instruction::Mod,
        "neg" => Instruction::Neg,
        "eq" => Instruction::Eq,
        "lt" => Instruction::Lt,
        "gt" => Instruction::Gt,
        "le" => Instruction::Le,
        "ge" => Instruction::Ge,
        "not" => Instruction::Not,
        "load" => Instruction::Load(name_arg(arg)?),
        "store" => Instruction::Store(name_arg(arg)?),
        "jmp" => Instruction::Jmp(target_arg(arg)?),
        "jz" => Instruction::Jz(target_arg(arg)?),
        "jnz" => Instruction::Jnz(target_arg(arg)?),
        "call" => Instruction::Call {
            target: target_arg(arg)?,
            name: arg.unwrap().to_string(),
        },
        "ret" => Instruction::Ret,
        "mstore" => Instruction::MStore,
        "mload" => Instruction::MLoad,
        "print" => Instruction::Print,
        "halt" => Instruction::Halt,
        other => return Err(err(format!("unknown instruction `{other}`"))),
    };
    Ok(instr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_labels_and_jumps() {
        let src = "\
            push 3\n\
        loop:\n\
            dup\n\
            print\n\
            push 1\n\
            sub\n\
            dup\n\
            jnz loop\n\
            halt\n";
        let p = assemble(src).expect("should assemble");
        // `loop` points at the `dup` which is instruction index 1.
        assert_eq!(p.code[1], Instruction::Dup);
        // Instructions: 0 push, 1 dup, 2 print, 3 push, 4 sub, 5 dup, 6 jnz, 7 halt.
        match &p.code[6] {
            Instruction::Jnz(t) => assert_eq!(*t, 1),
            other => panic!("expected jnz, got {other:?}"),
        }
        assert_eq!(p.code[7], Instruction::Halt);
    }

    #[test]
    fn reports_unknown_label() {
        let e = assemble("jmp nowhere\nhalt\n").unwrap_err();
        assert!(e.message.contains("unknown label"));
    }

    #[test]
    fn trailing_label_targets_end() {
        let src = "\
            push 0\n\
            jz done\n\
            push 99\n\
            print\n\
        done:\n";
        let p = assemble(src).unwrap();
        match &p.code[1] {
            // `done:` has no following instruction, so it targets code.len() (4).
            Instruction::Jz(t) => assert_eq!(*t, p.code.len()),
            other => panic!("expected jz, got {other:?}"),
        }
    }
}
