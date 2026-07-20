//! A tiny query language for searching the timeline.
//!
//! You type a condition in the debugger and chronovm jumps to the first step
//! where it holds. Supported forms:
//!
//! ```text
//!   acc > 100        ; a variable compared to a number
//!   n == 0           ; == != < > <= >= all work
//!   depth >= 4       ; `depth` = call-stack depth (find deep recursion)
//!   top < 0          ; `top` = the value on top of the stack
//!   fault            ; the step where execution faulted
//! ```
//!
//! Variable lookups scan the call stack from the innermost frame outward, so a
//! local that only exists inside a function is still findable while you are in
//! (or below) that call.

use crate::vm::Frame;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Operand {
    Depth,
    Top,
    Var(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CmpOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

/// A parsed, evaluable search condition.
#[derive(Clone, Debug, PartialEq)]
pub enum Predicate {
    Fault,
    Cmp { lhs: Operand, op: CmpOp, rhs: i64 },
}

/// Parse a query string into a [`Predicate`], or return a human-readable error.
pub fn parse(input: &str) -> Result<Predicate, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("empty query".into());
    }
    if s.eq_ignore_ascii_case("fault") {
        return Ok(Predicate::Fault);
    }

    // Find the comparison operator; check two-char operators first so ">=" is
    // not mistaken for ">".
    let (op, at, width) = ["==", "!=", "<=", ">=", "<", ">", "="]
        .iter()
        .find_map(|token| s.find(token).map(|i| (*token, i, token.len())))
        .ok_or_else(|| {
            "expected a condition like `acc > 100`, `n == 0`, `depth >= 3`, or `fault`".to_string()
        })?;

    let lhs_str = s[..at].trim();
    let rhs_str = s[at + width..].trim();
    if lhs_str.is_empty() || rhs_str.is_empty() {
        return Err("condition needs a name on the left and a number on the right".into());
    }

    let op = match op {
        "==" | "=" => CmpOp::Eq,
        "!=" => CmpOp::Ne,
        "<" => CmpOp::Lt,
        ">" => CmpOp::Gt,
        "<=" => CmpOp::Le,
        ">=" => CmpOp::Ge,
        _ => unreachable!(),
    };

    let lhs = match lhs_str {
        "depth" => Operand::Depth,
        "top" => Operand::Top,
        name => Operand::Var(name.to_string()),
    };

    let rhs: i64 = rhs_str
        .parse()
        .map_err(|_| format!("`{rhs_str}` is not an integer"))?;

    Ok(Predicate::Cmp { lhs, op, rhs })
}

impl Predicate {
    /// Does this condition hold at the given frame?
    pub fn holds(&self, frame: &Frame) -> bool {
        match self {
            Predicate::Fault => frame.error.is_some(),
            Predicate::Cmp { lhs, op, rhs } => match resolve(lhs, frame) {
                Some(v) => op.apply(v, *rhs),
                None => false,
            },
        }
    }
}

impl CmpOp {
    fn apply(self, a: i64, b: i64) -> bool {
        match self {
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
            CmpOp::Lt => a < b,
            CmpOp::Gt => a > b,
            CmpOp::Le => a <= b,
            CmpOp::Ge => a >= b,
        }
    }
}

/// Resolve an operand to a concrete value at a frame, or `None` if it isn't
/// defined there (e.g. a variable that doesn't exist yet).
fn resolve(operand: &Operand, frame: &Frame) -> Option<i64> {
    match operand {
        Operand::Depth => Some(frame.call_stack.len() as i64),
        Operand::Top => frame.stack.last().copied(),
        Operand::Var(name) => frame
            .call_stack
            .iter()
            .rev()
            .find_map(|scope| scope.locals.get(name).copied()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assembler::assemble, vm::record};

    #[test]
    fn parses_the_supported_forms() {
        assert_eq!(parse("fault").unwrap(), Predicate::Fault);
        assert!(matches!(parse("acc > 100"), Ok(Predicate::Cmp { .. })));
        assert!(matches!(parse("n==0"), Ok(Predicate::Cmp { .. })));
        assert!(matches!(parse("depth >= 3"), Ok(Predicate::Cmp { .. })));
        assert!(parse("").is_err());
        assert!(parse("acc !!! 3").is_err());
        assert!(parse("acc > seven").is_err());
    }

    #[test]
    fn finds_the_step_where_a_variable_crosses_a_threshold() {
        // acc runs 1, 1, 2, 6, 24, 120 across the iterative factorial.
        let t = record(assemble(include_str!("../examples/factorial.cvm")).unwrap());
        let pred = parse("acc >= 100").unwrap();
        let first = (0..=t.last()).find(|&i| pred.holds(&t.frames[i]));
        assert!(first.is_some(), "acc should reach 100 (it hits 120)");
        // At that step acc must really be >= 100.
        let f = &t.frames[first.unwrap()];
        assert!(f.vars().get("acc").copied().unwrap() >= 100);
    }

    #[test]
    fn depth_query_finds_deep_recursion() {
        let t = record(assemble(include_str!("../examples/recursive.cvm")).unwrap());
        let pred = parse("depth >= 5").unwrap();
        assert!(
            (0..=t.last()).any(|i| pred.holds(&t.frames[i])),
            "recursive factorial(5) should reach call depth 5"
        );
    }

    #[test]
    fn fault_query_finds_the_fault() {
        let t = record(assemble(include_str!("../examples/buggy.cvm")).unwrap());
        let pred = parse("fault").unwrap();
        let hits: Vec<usize> = (0..=t.last())
            .filter(|&i| pred.holds(&t.frames[i]))
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "exactly the final faulting step should match"
        );
    }
}
