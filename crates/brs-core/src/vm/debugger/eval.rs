//! Debug-console statements use the language parser and normal value operations.
use super::*;
use crate::ast::{Expr, LValue, Literal, PrintItem, Stmt, StmtKind};
use std::collections::HashMap;

impl Vm {
    pub(super) fn debug_eval(&mut self, code: &str) -> Result<Value, String> {
        let parsed = crate::parser::parse_source(&format!("sub __console()\n{code}\nend sub"))
            .map_err(|e| e.to_string())?;
        let mut budget = 10_000;
        self.debug_eval_active = true;
        let result = self.eval_statements(&parsed[0].body, &mut HashMap::new(), &mut budget);
        self.debug_eval_active = false;
        result
    }

    fn eval_statements(
        &mut self,
        statements: &[Stmt],
        locals: &mut HashMap<String, Value>,
        budget: &mut usize,
    ) -> Result<Value, String> {
        let mut result = Value::Invalid;
        for statement in statements {
            if *budget == 0 {
                return Err("Debugger evaluation budget exhausted".into());
            }
            *budget -= 1;
            match &statement.kind {
                StmtKind::Expression(expr) => result = self.eval_expression(expr, locals)?,
                StmtKind::Return(expr) => {
                    return expr
                        .as_ref()
                        .map_or(Ok(Value::Invalid), |e| self.eval_expression(e, locals))
                }
                StmtKind::Assign { target, op, value } => {
                    let mut value = self.eval_expression(value, locals)?;
                    if let Some(op) = op {
                        let expr = match target {
                            LValue::Variable(n) => Expr::Identifier(n.clone()),
                            LValue::Dot(o, n) => Expr::Dot(Box::new(o.clone()), n.clone()),
                            LValue::Index(o, i) => Expr::Index(Box::new(o.clone()), i.clone()),
                        };
                        value = ops::binary(*op, &self.eval_expression(&expr, locals)?, &value)
                            .map_err(|e| e.to_string())?;
                    }
                    match target {
                        LValue::Variable(name) => {
                            if !self.eval_set_local(name, value.clone()) {
                                locals.insert(name.to_lowercase(), value);
                            }
                        }
                        LValue::Dot(object, name) => {
                            let object = self.eval_expression(object, locals)?;
                            match crate::sg::node_ref(&object) {
                                Some(node) => {
                                    crate::sg::runtime::set_field(self, &node, name, value)
                                }
                                None => set_field(&object, name, value),
                            }
                            .map_err(|e| e.to_string())?
                        }
                        LValue::Index(object, indexes) => {
                            let object = self.eval_expression(object, locals)?;
                            let indexes = indexes
                                .iter()
                                .map(|e| self.eval_expression(e, locals))
                                .collect::<Result<Vec<_>, _>>()?;
                            match (crate::sg::node_ref(&object), indexes.as_slice()) {
                                (Some(node), [Value::String(key)]) => {
                                    crate::sg::runtime::set_field(self, &node, key, value)
                                }
                                _ => set_index(&object, &indexes, value),
                            }
                            .map_err(|e| e.to_string())?;
                        }
                    }
                }
                StmtKind::Print(items) => {
                    for item in items {
                        match item {
                            PrintItem::Expr(expr) => {
                                let value = self.eval_expression(expr, locals)?;
                                self.write(&to_print_string(&value));
                            }
                            PrintItem::Comma => {
                                let count = 16 - self.column % 16;
                                self.write(&" ".repeat(count));
                            }
                            PrintItem::Semicolon => {}
                        }
                    }
                    if !matches!(items.last(), Some(PrintItem::Comma | PrintItem::Semicolon)) {
                        self.write("\n");
                    }
                }
                StmtKind::ForEach {
                    variable,
                    iterable,
                    body,
                } => {
                    let items = ops::iteration_items(&self.eval_expression(iterable, locals)?)
                        .map_err(|e| e.to_string())?;
                    for item in items {
                        locals.insert(variable.to_lowercase(), item);
                        self.eval_statements(body, locals, budget)?;
                    }
                }
                StmtKind::If {
                    branches,
                    otherwise,
                } => {
                    let mut matched = false;
                    for (condition, body) in branches {
                        if ops::condition(&self.eval_expression(condition, locals)?)
                            .map_err(|e| e.to_string())?
                        {
                            result = self.eval_statements(body, locals, budget)?;
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        if let Some(body) = otherwise {
                            result = self.eval_statements(body, locals, budget)?;
                        }
                    }
                }
                _ => return Err("Unsupported console statement".into()),
            }
        }
        Ok(result)
    }

    fn eval_frame_index(&self) -> Option<usize> {
        self.frames.len().checked_sub(1 + self.debug_frame)
    }
    fn eval_set_local(&mut self, name: &str, value: Value) -> bool {
        let Some(frame) = self.eval_frame_index().and_then(|i| self.frames.get(i)) else {
            return false;
        };
        let proto = &self.program.functions[frame.function as usize];
        if let Some(slot) = proto
            .locals
            .iter()
            .position(|n| n.eq_ignore_ascii_case(name))
        {
            self.stack[frame.base + slot] = value;
            true
        } else {
            false
        }
    }
    pub(super) fn eval_expression(
        &mut self,
        expr: &Expr,
        locals: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        Ok(match expr {
            Expr::Literal(lit) => match lit {
                Literal::Invalid => Value::Invalid,
                Literal::Boolean(v) => Value::Boolean(*v),
                Literal::Integer(v) => Value::Integer(*v),
                Literal::LongInteger(v) => Value::LongInteger(*v),
                Literal::Float(v) => Value::Float(*v),
                Literal::Double(v) => Value::Double(*v),
                Literal::String(v) => Value::String(v.clone()),
            },
            Expr::Identifier(name) => {
                let key = name.to_lowercase();
                if let Some(value) = locals.get(&key) {
                    value.clone()
                } else {
                    let frame = self
                        .eval_frame_index()
                        .and_then(|i| self.frames.get(i))
                        .ok_or("No selected frame")?;
                    let proto = &self.program.functions[frame.function as usize];
                    if key == "m" {
                        frame.m.clone()
                    } else if let Some(slot) = proto
                        .locals
                        .iter()
                        .position(|n| n.eq_ignore_ascii_case(name))
                    {
                        self.stack[frame.base + slot].clone()
                    } else if let Some(function) = self.program.lookup(proto.scope, &key) {
                        Value::Function(FunctionRef::User(function))
                    } else if let Some(function) = builtins::lookup(&key) {
                        Value::Function(FunctionRef::Builtin(function))
                    } else {
                        return Err(format!("Unknown variable: {name}"));
                    }
                }
            }
            Expr::Binary(op, left, right) => {
                let left = self.eval_expression(left, locals)?;
                if *op == crate::ast::BinaryOp::And && matches!(left, Value::Boolean(false)) {
                    return Ok(left);
                }
                if *op == crate::ast::BinaryOp::Or && matches!(left, Value::Boolean(true)) {
                    return Ok(left);
                }
                let right = self.eval_expression(right, locals)?;
                ops::binary(*op, &left, &right).map_err(|e| e.to_string())?
            }
            Expr::Unary(op, value) => {
                ops::unary(*op, &self.eval_expression(value, locals)?).map_err(|e| e.to_string())?
            }
            Expr::OptionalDot(object, name) => {
                let object = self.eval_expression(object, locals)?;
                if object.is_invalid() {
                    Value::Invalid
                } else {
                    get_field(&object, name).map_err(|e| e.to_string())?
                }
            }
            Expr::Dot(object, name) => get_field(&self.eval_expression(object, locals)?, name)
                .map_err(|e| e.to_string())?,
            Expr::Index(object, indexes) => {
                let object = self.eval_expression(object, locals)?;
                let indexes = indexes
                    .iter()
                    .map(|e| self.eval_expression(e, locals))
                    .collect::<Result<Vec<_>, _>>()?;
                get_index(&object, &indexes).map_err(|e| e.to_string())?
            }
            Expr::Call(target, args) => {
                let args = args
                    .iter()
                    .map(|e| self.eval_expression(e, locals))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Expr::Dot(object, name) = target.as_ref() {
                    let object = self.eval_expression(object, locals)?;
                    builtins::call_method(self, &object, &name.to_lowercase(), args)
                        .map_err(|e| e.to_string())?
                } else {
                    match self.eval_expression(target, locals)? {
                        Value::Function(FunctionRef::Builtin(index)) => {
                            builtins::call(self, index, args).map_err(|e| e.to_string())?
                        }
                        Value::Function(FunctionRef::User(index)) => {
                            let saved_crashed = self.crashed;
                            let saved_status = self.status;
                            let saved_step = self.debug_step.take();
                            let m = self
                                .eval_frame_index()
                                .map(|i| self.frames[i].m.clone())
                                .unwrap_or(Value::Invalid);
                            let result = self.call_sync(index, args, m).map_err(|e| e.to_string());
                            self.crashed = saved_crashed;
                            self.status = saved_status;
                            self.debug_step = saved_step;
                            result?
                        }
                        _ => return Err("Not a function".into()),
                    }
                }
            }
            Expr::ArrayLiteral(items) => Value::object(Object::Array(
                items
                    .iter()
                    .map(|e| self.eval_expression(e, locals))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Expr::AssocArrayLiteral(items) => {
                let mut values = AssocArray::new();
                for (key, expr) in items {
                    values.set(key, self.eval_expression(expr, locals)?);
                }
                Value::object(Object::AssocArray(values))
            }
            Expr::Function(_) => {
                return Err("Function declarations are not allowed in console expressions".into())
            }
        })
    }
}
