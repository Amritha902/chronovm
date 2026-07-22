//! Browser bindings: expose the recording VM to JavaScript via wasm-bindgen.
//!
//! The whole point of the library split is that the web UI reuses the *exact*
//! same VM, causal engine, and query language as the terminal. This module is
//! just a thin serialization layer — it holds the recorded [`Trace`] in wasm
//! memory and hands the browser plain JSON views on demand.

use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::vm::{Frame, Trace};
use crate::{assembler, query, vm};

/// A recorded program. Created once from source; the UI then pulls frames,
/// causal chains, and search results out of it as the user scrubs.
#[wasm_bindgen]
pub struct Session {
    trace: Trace,
}

#[derive(Serialize)]
struct InstrView {
    text: String,
    mnemonic: String,
    labels: Vec<String>,
}

#[derive(Serialize)]
struct ProgramView {
    instructions: Vec<InstrView>,
}

#[derive(Serialize)]
struct StackSlot {
    value: i64,
    origin: usize,
}

#[derive(Serialize)]
struct VarView {
    name: String,
    value: i64,
    def: usize,
}

#[derive(Serialize)]
struct ScopeView {
    func: String,
    locals: Vec<VarView>,
}

#[derive(Serialize)]
struct FrameView {
    ip: usize,
    last_ip: Option<usize>,
    last_op: Option<String>,
    stack: Vec<StackSlot>,
    /// Bottom (main) to top (current) — the same order as the terminal panel.
    call_stack: Vec<ScopeView>,
    /// Linear memory contents (empty unless the program uses mstore/mload).
    memory: Vec<i64>,
    output: String,
    error: Option<String>,
    halted: bool,
    wrote_var: Option<String>,
}

#[derive(Serialize)]
struct CausalNodeView {
    step: usize,
    description: String,
}

#[derive(Serialize)]
struct SearchResult {
    matches: Vec<usize>,
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsError> {
    serde_wasm_bindgen::to_value(value).map_err(|e| JsError::new(&e.to_string()))
}

fn frame_view(f: &Frame, output: &str) -> FrameView {
    FrameView {
        ip: f.ip,
        last_ip: f.last_ip,
        last_op: f.last_op.as_ref().map(|o| o.mnemonic()),
        stack: f
            .stack
            .iter()
            .zip(f.stack_origin.iter())
            .map(|(&value, &origin)| StackSlot { value, origin })
            .collect(),
        call_stack: f
            .call_stack
            .iter()
            .map(|scope| ScopeView {
                func: scope.func.clone(),
                locals: scope
                    .locals
                    .iter()
                    .map(|(name, &value)| VarView {
                        name: name.clone(),
                        value,
                        def: scope.locals_def.get(name).copied().unwrap_or(0),
                    })
                    .collect(),
            })
            .collect(),
        memory: f.memory.as_ref().clone(),
        output: output.to_string(),
        error: f.error.clone(),
        halted: f.halted,
        wrote_var: f.wrote_var.clone(),
    }
}

#[wasm_bindgen]
impl Session {
    /// Assemble and record a program. Throws a JS error with a readable message
    /// if the source fails to assemble.
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str) -> Result<Session, JsError> {
        let program = assembler::assemble(source).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Session {
            trace: vm::record(program),
        })
    }

    /// Number of recorded frames (frame 0 is the initial state).
    #[wasm_bindgen(js_name = frameCount)]
    pub fn frame_count(&self) -> usize {
        self.trace.frames.len()
    }

    /// The program listing: one entry per instruction, with source text,
    /// normalized mnemonic, and any labels pointing at it.
    pub fn program(&self) -> Result<JsValue, JsError> {
        let p = &self.trace.program;
        let instructions = (0..p.len())
            .map(|i| InstrView {
                text: p.source[i].clone(),
                mnemonic: p.code[i].mnemonic(),
                labels: p.labels_at[i].clone(),
            })
            .collect();
        to_js(&ProgramView { instructions })
    }

    /// The full machine state at frame `i`.
    pub fn frame(&self, i: usize) -> Result<JsValue, JsError> {
        let f = self
            .trace
            .frames
            .get(i)
            .ok_or_else(|| JsError::new("frame index out of range"))?;
        to_js(&frame_view(f, self.trace.output_at(i)))
    }

    /// The causal chain explaining variable `var` as of frame `i`.
    #[wasm_bindgen(js_name = explainVar)]
    pub fn explain_var(&self, i: usize, var: &str) -> Result<JsValue, JsError> {
        let chain: Vec<CausalNodeView> = self
            .trace
            .explain_var(i, var)
            .into_iter()
            .map(|n| CausalNodeView {
                step: n.step,
                description: n.description,
            })
            .collect();
        to_js(&chain)
    }

    /// The causal chain for whatever a given step consumed — the engine behind
    /// "why did it crash?", seeded from the faulting step.
    #[wasm_bindgen(js_name = explainStep)]
    pub fn explain_step(&self, step: usize) -> Result<JsValue, JsError> {
        let chain: Vec<CausalNodeView> = self
            .trace
            .explain_step(step, 32)
            .into_iter()
            .map(|n| CausalNodeView {
                step: n.step,
                description: n.description,
            })
            .collect();
        to_js(&chain)
    }

    /// Every step where a search condition holds. Throws on a malformed query.
    pub fn search(&self, q: &str) -> Result<JsValue, JsError> {
        let pred = query::parse(q).map_err(|e| JsError::new(&e))?;
        let matches = (0..self.trace.frames.len())
            .filter(|&i| pred.holds(&self.trace.frames[i]))
            .collect();
        to_js(&SearchResult { matches })
    }
}
