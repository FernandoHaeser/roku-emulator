//! AST -> bytecode. Variables are function-scoped and case-insensitive; each
//! one gets a fixed slot in its frame.

use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::ast::*;
use crate::builtins;
use crate::bytecode::{FunctionProto, Instr, Label, ParamInfo, Program};
use crate::value::{FunctionRef, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct CompileError {
    pub message: String,
    pub file: String,
    pub line: u32,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} in {}({})", self.message, self.file, self.line)
    }
}

/// A parsed source file ready to be compiled.
pub struct SourceFile {
    /// Device path, e.g. "pkg:/source/main.brs".
    pub path: Rc<str>,
    pub text: Rc<str>,
    pub functions: Vec<FunctionDecl>,
}

pub fn compile_program(files: Vec<SourceFile>) -> Result<Program, CompileError> {
    let mut program = Program::default();
    compile_scope(&mut program, &[&files])?;
    program.function_index = program.scopes[0].clone();
    Ok(program)
}

/// Functions declared by the most derived level of a scope (name -> index).
pub type OwnFunctions = HashMap<String, u32>;

/// Compiles one function namespace into `program` and returns its id plus the
/// functions declared by the last level. `levels` lists the files of each
/// component in an `extends` chain, base first: a level may override a
/// function of an earlier level, but not redefine one of its own.
pub fn compile_scope(
    program: &mut Program,
    levels: &[&[SourceFile]],
) -> Result<(u32, OwnFunctions), CompileError> {
    let scope = program.scopes.len() as u32;
    program.scopes.push(HashMap::new());
    let mut own = HashMap::new();
    // Pass 1: register every named function so calls resolve across files.
    let mut pending = Vec::new();
    for (level_number, files) in levels.iter().enumerate() {
        let mut level_names = HashMap::new();
        for file in files.iter() {
            program.sources.insert(file.path.clone(), file.text.clone());
            for decl in &file.functions {
                let name = decl.name.clone().unwrap_or_default();
                let key = name.to_lowercase();
                if level_names.insert(key.clone(), ()).is_some() {
                    return Err(CompileError {
                        message: format!("Function Re-defined: {name}"),
                        file: file.path.to_string(),
                        line: decl.line,
                    });
                }
                let index = program.functions.len() as u32;
                program.scopes[scope as usize].insert(key.clone(), index);
                if level_number + 1 == levels.len() {
                    own.insert(key, index);
                }
                let mut proto = placeholder(&name, &file.path, decl);
                proto.scope = scope;
                program.functions.push(proto);
                pending.push((index, file.path.clone(), decl));
            }
        }
    }
    // Pass 2: compile bodies.
    let mut shared = Shared {
        program,
        string_constants: HashMap::new(),
        anonymous_count: 0,
        scope,
    };
    for (index, path, decl) in pending {
        let proto =
            shared.compile_function(decl, path.clone(), decl.name.clone().unwrap_or_default())?;
        shared.program.functions[index as usize] = proto;
    }
    Ok((scope, own))
}

fn placeholder(name: &str, path: &Rc<str>, decl: &FunctionDecl) -> FunctionProto {
    FunctionProto {
        name: name.to_string(),
        is_sub: decl.is_sub,
        params: Vec::new(),
        return_type: None,
        code: Vec::new(),
        lines: Vec::new(),
        locals: Vec::new(),
        file: path.clone(),
        first_line: decl.line,
        last_line: decl.end_line,
        scope: 0,
    }
}

struct Shared<'p> {
    program: &'p mut Program,
    string_constants: HashMap<String, u32>,
    anonymous_count: u32,
    scope: u32,
}

impl Shared<'_> {
    fn constant(&mut self, value: Value) -> u32 {
        if let Value::String(text) = &value {
            if let Some(&index) = self.string_constants.get(text.as_ref()) {
                return index;
            }
            let index = self.program.constants.len() as u32;
            self.string_constants.insert(text.to_string(), index);
            self.program.constants.push(value);
            return index;
        }
        self.program.constants.push(value);
        (self.program.constants.len() - 1) as u32
    }

    fn compile_function(
        &mut self,
        decl: &FunctionDecl,
        file: Rc<str>,
        name: String,
    ) -> Result<FunctionProto, CompileError> {
        let scope = self.scope;
        let mut compiler = FunctionCompiler {
            shared: self,
            proto: FunctionProto {
                name,
                is_sub: decl.is_sub,
                params: Vec::new(),
                return_type: decl.return_type,
                code: Vec::new(),
                lines: Vec::new(),
                locals: Vec::new(),
                file,
                first_line: decl.line,
                last_line: decl.end_line,
                scope,
            },
            slots: HashMap::new(),
            loops: Vec::new(),
            line: decl.line,
            hidden_count: 0,
        };
        compiler.prologue(decl)?;
        compiler.block(&decl.body)?;
        compiler.emit(Instr::Return);
        Ok(compiler.proto)
    }
}

#[derive(PartialEq)]
enum LoopKind {
    For,
    While,
}

struct LoopContext {
    kind: LoopKind,
    exits: Vec<usize>,
    continue_target: Option<Label>,
    continues: Vec<usize>,
}

struct FunctionCompiler<'a, 'p> {
    shared: &'a mut Shared<'p>,
    proto: FunctionProto,
    slots: HashMap<String, u16>,
    loops: Vec<LoopContext>,
    line: u32,
    hidden_count: u32,
}

impl FunctionCompiler<'_, '_> {
    fn error<T>(&self, message: impl Into<String>) -> Result<T, CompileError> {
        Err(CompileError {
            message: message.into(),
            file: self.proto.file.to_string(),
            line: self.line,
        })
    }

    fn emit(&mut self, instr: Instr) -> usize {
        self.proto.code.push(instr);
        self.proto.lines.push(self.line);
        self.proto.code.len() - 1
    }

    fn here(&self) -> Label {
        self.proto.code.len() as Label
    }

    fn patch(&mut self, at: usize, target: Label) {
        match &mut self.proto.code[at] {
            Instr::Jump(label)
            | Instr::JumpIfFalse(label)
            | Instr::AndLeft(label)
            | Instr::OrLeft(label)
            | Instr::TryBegin(label)
            | Instr::JumpIfInitialized(_, label) => *label = target,
            Instr::ForCheck { exit, .. } | Instr::IterNext { exit, .. } => *exit = target,
            other => unreachable!("cannot patch {other:?}"),
        }
    }

    fn slot(&mut self, name: &str) -> u16 {
        let key = name.to_lowercase();
        if let Some(&slot) = self.slots.get(&key) {
            return slot;
        }
        let slot = self.proto.locals.len() as u16;
        self.proto.locals.push(name.to_string());
        self.slots.insert(key, slot);
        slot
    }

    fn hidden_slot(&mut self, purpose: &str) -> u16 {
        self.hidden_count += 1;
        let name = format!("${purpose}{}", self.hidden_count);
        self.slot(&name)
    }

    fn constant(&mut self, value: Value) -> u32 {
        self.shared.constant(value)
    }

    fn name_constant(&mut self, name: &str) -> u32 {
        self.constant(Value::string(name))
    }

    fn prologue(&mut self, decl: &FunctionDecl) -> Result<(), CompileError> {
        for param in &decl.params {
            let slot = self.slot(&param.name);
            self.proto.params.push(ParamInfo {
                name: param.name.clone(),
                slot,
                has_default: param.default.is_some(),
            });
        }
        for param in &decl.params {
            let slot = self.slots[&param.name.to_lowercase()];
            if let Some(default) = &param.default {
                let skip = self.emit(Instr::JumpIfInitialized(slot, 0));
                self.expression(default)?;
                self.emit(Instr::StoreLocal(slot));
                let target = self.here();
                self.patch(skip, target);
            }
            if let Some(ty) = param.ty {
                if !matches!(ty, TypeName::Dynamic | TypeName::Object) {
                    self.emit(Instr::CoerceLocal(slot, ty));
                }
            }
        }
        Ok(())
    }

    // ---- statements --------------------------------------------------------

    fn block(&mut self, statements: &[Stmt]) -> Result<(), CompileError> {
        for statement in statements {
            self.statement(statement)?;
        }
        Ok(())
    }

    fn statement(&mut self, statement: &Stmt) -> Result<(), CompileError> {
        self.line = statement.line;
        match &statement.kind {
            StmtKind::Assign { target, op, value } => self.assign(target, *op, value)?,
            StmtKind::Increment { target, delta } => self.assign(
                target,
                Some(BinaryOp::Add),
                &Expr::Literal(Literal::Integer(*delta)),
            )?,
            StmtKind::Expression(expr) => {
                self.expression(expr)?;
                self.emit(Instr::Pop);
            }
            StmtKind::Print(items) => self.print(items)?,
            StmtKind::If {
                branches,
                otherwise,
            } => self.if_statement(branches, otherwise.as_deref())?,
            StmtKind::For {
                variable,
                start,
                end,
                step,
                body,
            } => self.for_loop(variable, start, end, step.as_ref(), body)?,
            StmtKind::ForEach {
                variable,
                iterable,
                body,
            } => self.for_each(variable, iterable, body)?,
            StmtKind::While { condition, body } => self.while_loop(condition, body)?,
            StmtKind::ExitFor => self.loop_jump(LoopKind::For, false)?,
            StmtKind::ExitWhile => self.loop_jump(LoopKind::While, false)?,
            StmtKind::ContinueFor => self.loop_jump(LoopKind::For, true)?,
            StmtKind::ContinueWhile => self.loop_jump(LoopKind::While, true)?,
            StmtKind::Return(value) => match value {
                Some(expr) => {
                    self.expression(expr)?;
                    self.emit(Instr::ReturnValue);
                }
                None => {
                    self.emit(Instr::Return);
                }
            },
            StmtKind::Dim { name, dimensions } => {
                for dimension in dimensions {
                    self.expression(dimension)?;
                }
                self.emit(Instr::NewDimArray(dimensions.len() as u8));
                let slot = self.slot(name);
                self.emit(Instr::StoreLocal(slot));
            }
            StmtKind::Try {
                body,
                error_variable,
                handler,
            } => {
                let begin = self.emit(Instr::TryBegin(0));
                self.block(body)?;
                self.emit(Instr::TryEnd);
                let skip = self.emit(Instr::Jump(0));
                let catch_target = self.here();
                self.patch(begin, catch_target);
                let slot = self.slot(error_variable);
                self.emit(Instr::StoreLocal(slot));
                self.block(handler)?;
                let end = self.here();
                self.patch(skip, end);
            }
            StmtKind::Throw(value) => {
                self.expression(value)?;
                self.emit(Instr::Throw);
            }
            StmtKind::Stop => {
                self.emit(Instr::Stop);
            }
            StmtKind::End => {
                self.emit(Instr::End);
            }
        }
        Ok(())
    }

    fn assign(
        &mut self,
        target: &LValue,
        op: Option<BinaryOp>,
        value: &Expr,
    ) -> Result<(), CompileError> {
        match target {
            LValue::Variable(name) => {
                if name.eq_ignore_ascii_case("m") {
                    return self.error("Syntax Error: cannot assign to m");
                }
                let slot = self.slot(name);
                if let Some(op) = op {
                    self.emit(Instr::LoadLocal(slot));
                    self.expression(value)?;
                    self.emit(Instr::Binary(op));
                } else {
                    self.expression(value)?;
                }
                self.emit(Instr::StoreLocal(slot));
            }
            LValue::Dot(object, name) => {
                self.expression(object)?;
                // Dot access lowercases the key, like AA literal keys.
                let key = self.name_constant(&name.to_lowercase());
                if let Some(op) = op {
                    self.emit(Instr::Dup);
                    self.emit(Instr::GetField(key));
                    self.expression(value)?;
                    self.emit(Instr::Binary(op));
                } else {
                    self.expression(value)?;
                }
                self.emit(Instr::SetField(key));
            }
            LValue::Index(object, indexes) => {
                self.expression(object)?;
                for index in indexes {
                    self.expression(index)?;
                }
                if let Some(op) = op {
                    // Re-evaluates the target expressions to read the current value.
                    self.expression(object)?;
                    for index in indexes {
                        self.expression(index)?;
                    }
                    self.emit(Instr::GetIndex(indexes.len() as u8));
                    self.expression(value)?;
                    self.emit(Instr::Binary(op));
                } else {
                    self.expression(value)?;
                }
                self.emit(Instr::SetIndex(indexes.len() as u8));
            }
        }
        Ok(())
    }

    fn print(&mut self, items: &[PrintItem]) -> Result<(), CompileError> {
        // The device evaluates every expression before writing anything: an
        // error or diagnostic raised by a later item appears before the line.
        let expressions: Vec<&Expr> = items
            .iter()
            .filter_map(|item| {
                if let PrintItem::Expr(expr) = item {
                    Some(expr)
                } else {
                    None
                }
            })
            .collect();
        if expressions.len() > u8::MAX as usize {
            return self.error("Syntax Error: too many PRINT items");
        }
        for expr in &expressions {
            self.expression(expr)?;
        }
        let count = expressions.len() as u8;
        let mut index = 0u8;
        for item in items {
            match item {
                PrintItem::Expr(_) => {
                    self.emit(Instr::PrintStacked { index, count });
                    index += 1;
                }
                PrintItem::Comma => {
                    self.emit(Instr::PrintComma);
                }
                PrintItem::Semicolon => {}
            }
        }
        if count > 0 {
            self.emit(Instr::PopN(count));
        }
        // A trailing separator keeps the cursor on the same line.
        if !matches!(items.last(), Some(PrintItem::Comma | PrintItem::Semicolon)) {
            self.emit(Instr::PrintNewline);
        }
        Ok(())
    }

    fn if_statement(
        &mut self,
        branches: &[(Expr, Vec<Stmt>)],
        otherwise: Option<&[Stmt]>,
    ) -> Result<(), CompileError> {
        let mut end_jumps = Vec::new();
        for (condition, body) in branches {
            self.expression(condition)?;
            let skip = self.emit(Instr::JumpIfFalse(0));
            self.block(body)?;
            end_jumps.push(self.emit(Instr::Jump(0)));
            let next = self.here();
            self.patch(skip, next);
        }
        if let Some(body) = otherwise {
            self.block(body)?;
        }
        let end = self.here();
        for jump in end_jumps {
            self.patch(jump, end);
        }
        Ok(())
    }

    fn for_loop(
        &mut self,
        variable: &str,
        start: &Expr,
        end: &Expr,
        step: Option<&Expr>,
        body: &[Stmt],
    ) -> Result<(), CompileError> {
        let variable_slot = self.slot(variable);
        self.expression(start)?;
        self.emit(Instr::StoreLocal(variable_slot));
        let end_slot = self.hidden_slot("end");
        self.expression(end)?;
        self.emit(Instr::StoreLocal(end_slot));
        let step_slot = self.hidden_slot("step");
        match step {
            Some(step) => self.expression(step)?,
            None => {
                let one = self.constant(Value::Integer(1));
                self.emit(Instr::Constant(one));
            }
        }
        self.emit(Instr::StoreLocal(step_slot));

        let top = self.here();
        let check = self.emit(Instr::ForCheck {
            variable: variable_slot,
            end: end_slot,
            step: step_slot,
            exit: 0,
        });
        self.loops.push(LoopContext {
            kind: LoopKind::For,
            exits: Vec::new(),
            continue_target: None,
            continues: Vec::new(),
        });
        self.block(body)?;
        let increment = self.here();
        self.emit(Instr::LoadLocal(variable_slot));
        self.emit(Instr::LoadLocal(step_slot));
        self.emit(Instr::Binary(BinaryOp::Add));
        self.emit(Instr::StoreLocal(variable_slot));
        self.emit(Instr::Jump(top));
        let exit = self.here();
        self.patch(check, exit);
        self.close_loop(increment, exit);
        Ok(())
    }

    fn for_each(
        &mut self,
        variable: &str,
        iterable: &Expr,
        body: &[Stmt],
    ) -> Result<(), CompileError> {
        self.expression(iterable)?;
        let iterator = self.hidden_slot("iter");
        self.emit(Instr::IterInit(iterator));
        let variable_slot = self.slot(variable);
        let top = self.here();
        let next = self.emit(Instr::IterNext {
            iterator,
            variable: variable_slot,
            exit: 0,
        });
        self.loops.push(LoopContext {
            kind: LoopKind::For,
            exits: Vec::new(),
            continue_target: Some(top),
            continues: Vec::new(),
        });
        self.block(body)?;
        self.emit(Instr::Jump(top));
        let exit = self.here();
        self.patch(next, exit);
        self.close_loop(top, exit);
        Ok(())
    }

    fn while_loop(&mut self, condition: &Expr, body: &[Stmt]) -> Result<(), CompileError> {
        let top = self.here();
        self.expression(condition)?;
        let check = self.emit(Instr::JumpIfFalse(0));
        self.loops.push(LoopContext {
            kind: LoopKind::While,
            exits: Vec::new(),
            continue_target: Some(top),
            continues: Vec::new(),
        });
        self.block(body)?;
        self.emit(Instr::Jump(top));
        let exit = self.here();
        self.patch(check, exit);
        self.close_loop(top, exit);
        Ok(())
    }

    fn close_loop(&mut self, continue_target: Label, exit: Label) {
        let context = self.loops.pop().expect("loop context");
        for jump in context.exits {
            self.patch(jump, exit);
        }
        for jump in context.continues {
            self.patch(jump, continue_target);
        }
    }

    fn loop_jump(&mut self, kind: LoopKind, is_continue: bool) -> Result<(), CompileError> {
        let Some(position) = self.loops.iter().rposition(|context| context.kind == kind) else {
            let what = match (kind, is_continue) {
                (LoopKind::For, false) => "EXIT FOR",
                (LoopKind::While, false) => "EXIT WHILE",
                (LoopKind::For, true) => "CONTINUE FOR",
                (LoopKind::While, true) => "CONTINUE WHILE",
            };
            return self.error(format!("Syntax Error: {what} outside of loop"));
        };
        let jump = self.emit(Instr::Jump(0));
        let context = &mut self.loops[position];
        match (is_continue, context.continue_target) {
            (true, Some(target)) => self.patch(jump, target),
            (true, None) => context.continues.push(jump),
            (false, _) => context.exits.push(jump),
        }
        Ok(())
    }

    // ---- expressions -------------------------------------------------------

    fn expression(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Literal(literal) => {
                let value = match literal {
                    Literal::Invalid => Value::Invalid,
                    Literal::Boolean(b) => Value::Boolean(*b),
                    Literal::Integer(v) => Value::Integer(*v),
                    Literal::LongInteger(v) => Value::LongInteger(*v),
                    Literal::Float(v) => Value::Float(*v),
                    Literal::Double(v) => Value::Double(*v),
                    Literal::String(text) => Value::String(text.clone()),
                };
                let index = self.constant(value);
                self.emit(Instr::Constant(index));
            }
            Expr::Identifier(name) => self.identifier(name),
            Expr::Binary(BinaryOp::And, left, right) => {
                self.expression(left)?;
                let short = self.emit(Instr::AndLeft(0));
                self.expression(right)?;
                self.emit(Instr::AndRight);
                let end = self.here();
                self.patch(short, end);
            }
            Expr::Binary(BinaryOp::Or, left, right) => {
                self.expression(left)?;
                let short = self.emit(Instr::OrLeft(0));
                self.expression(right)?;
                self.emit(Instr::OrRight);
                let end = self.here();
                self.patch(short, end);
            }
            Expr::Binary(op, left, right) => {
                self.expression(left)?;
                self.expression(right)?;
                self.emit(Instr::Binary(*op));
            }
            Expr::Unary(op, operand) => {
                self.expression(operand)?;
                self.emit(Instr::Unary(*op));
            }
            Expr::Call(callee, args) => {
                if let Expr::Dot(receiver, name) = callee.as_ref() {
                    self.expression(receiver)?;
                    for arg in args {
                        self.expression(arg)?;
                    }
                    let name = self.name_constant(name);
                    self.emit(Instr::CallMethod(name, args.len() as u8));
                } else {
                    self.expression(callee)?;
                    for arg in args {
                        self.expression(arg)?;
                    }
                    self.emit(Instr::Call(args.len() as u8));
                }
            }
            Expr::Index(object, indexes) => {
                self.expression(object)?;
                for index in indexes {
                    self.expression(index)?;
                }
                self.emit(Instr::GetIndex(indexes.len() as u8));
            }
            Expr::Dot(object, name) => {
                self.expression(object)?;
                let key = self.name_constant(&name.to_lowercase());
                self.emit(Instr::GetField(key));
            }
            Expr::ArrayLiteral(items) => {
                for item in items {
                    self.expression(item)?;
                }
                self.emit(Instr::NewArray(items.len() as u32));
            }
            Expr::AssocArrayLiteral(entries) => {
                for (key, value) in entries {
                    let key = self.name_constant(key);
                    self.emit(Instr::Constant(key));
                    self.expression(value)?;
                }
                self.emit(Instr::NewAssocArray(entries.len() as u32));
            }
            Expr::Function(decl) => {
                self.shared.anonymous_count += 1;
                let name = format!("$anon_{}", self.shared.anonymous_count);
                let index = self.shared.program.functions.len() as u32;
                self.shared
                    .program
                    .functions
                    .push(placeholder(&name, &self.proto.file, decl));
                let file = self.proto.file.clone();
                let proto = self.shared.compile_function(decl, file, name)?;
                self.shared.program.functions[index as usize] = proto;
                self.emit(Instr::LoadFunction(index));
            }
        }
        Ok(())
    }

    /// Resolution order: local variable, user function, built-in function;
    /// anything else is a (possibly never assigned) local variable.
    fn identifier(&mut self, name: &str) {
        let key = name.to_lowercase();
        if key == "m" {
            self.emit(Instr::LoadM);
        } else if let Some(&slot) = self.slots.get(&key) {
            self.emit(Instr::LoadLocal(slot));
        } else if let Some(&index) =
            self.shared.program.scopes[self.shared.scope as usize].get(&key)
        {
            self.emit(Instr::LoadFunction(index));
        } else if let Some(index) = builtins::lookup(&key) {
            self.emit(Instr::LoadBuiltin(index));
        } else {
            let slot = self.slot(name);
            self.emit(Instr::LoadLocal(slot));
        }
    }
}

/// Function reference for a resolved global name (used by `run`/tests).
pub fn resolve_function(program: &Program, name: &str) -> Option<FunctionRef> {
    program
        .function_index
        .get(&name.to_lowercase())
        .map(|&i| FunctionRef::User(i))
}
