//! Bytecode executed by the VM. A stack machine with explicit frames, so a
//! running program can be suspended between any two instructions (needed for
//! `wait`, `sleep`, Task threads and breakpoints) and resumed later.

use std::rc::Rc;

use crate::ast::{BinaryOp, TypeName, UnaryOp};
use crate::value::Value;

pub type Label = u32;

#[derive(Debug, Clone, PartialEq)]
pub enum Instr {
    Constant(u32),
    LoadLocal(u16),
    StoreLocal(u16),
    LoadFunction(u32),
    LoadBuiltin(u16),
    LoadM,
    Pop,
    Dup,
    Binary(BinaryOp),
    Unary(UnaryOp),
    /// Short-circuit AND/OR: boolean left operand may decide the result.
    AndLeft(Label),
    AndRight,
    OrLeft(Label),
    OrRight,
    Jump(Label),
    JumpIfFalse(Label),
    /// Skips a parameter's default-value code when the caller supplied it.
    JumpIfInitialized(u16, Label),
    CoerceLocal(u16, TypeName),
    Call(u8),
    /// Method call: receiver and args on the stack; operand is the name constant.
    CallMethod(u32, u8),
    GetField(u32),
    SetField(u32),
    GetIndex(u8),
    SetIndex(u8),
    NewArray(u32),
    /// Pops `n` (key, value) pairs; keys are string constants pushed first.
    NewAssocArray(u32),
    NewDimArray(u8),
    ForCheck {
        variable: u16,
        end: u16,
        step: u16,
        exit: Label,
    },
    IterInit(u16),
    IterNext {
        iterator: u16,
        variable: u16,
        exit: Label,
    },
    /// Prints the value `index` of the last `count` stacked print operands.
    PrintStacked {
        index: u8,
        count: u8,
    },
    PopN(u8),
    PrintComma,
    PrintNewline,
    TryBegin(Label),
    TryEnd,
    Throw,
    Return,
    ReturnValue,
    Stop,
    End,
}

#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub name: String,
    pub slot: u16,
    pub has_default: bool,
}

#[derive(Debug, Clone)]
pub struct FunctionProto {
    /// Declared name (original case); anonymous functions get a synthetic one.
    pub name: String,
    pub is_sub: bool,
    pub params: Vec<ParamInfo>,
    pub return_type: Option<TypeName>,
    pub code: Vec<Instr>,
    /// Source line for each instruction (same length as `code`).
    pub lines: Vec<u32>,
    /// Local variable names in slot order (original case). Hidden compiler
    /// temporaries start with '$' and are skipped in debugger listings.
    pub locals: Vec<String>,
    pub file: Rc<str>,
    pub first_line: u32,
    pub last_line: u32,
    /// Function namespace this function was compiled in (0 = channel
    /// `source/`, others = SceneGraph components). Names used by observers
    /// and `callFunc` resolve in this scope.
    pub scope: u32,
}

#[derive(Debug, Default)]
pub struct Program {
    pub functions: Vec<FunctionProto>,
    pub constants: Vec<Value>,
    /// Lowercased function name -> index in `functions` (named functions only).
    pub function_index: std::collections::HashMap<String, u32>,
    /// Source text of each file, for debugger listings.
    pub sources: std::collections::HashMap<Rc<str>, Rc<str>>,
    /// Per-scope function tables; scope 0 mirrors `function_index`.
    pub scopes: Vec<std::collections::HashMap<String, u32>>,
}

impl Program {
    pub fn lookup(&self, scope: u32, name: &str) -> Option<u32> {
        self.scopes
            .get(scope as usize)
            .and_then(|table| table.get(&name.to_lowercase()))
            .copied()
    }
}
