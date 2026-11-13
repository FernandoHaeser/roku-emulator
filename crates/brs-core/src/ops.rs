//! Operator semantics. Promotion rules follow device traces
//! (fidelity/language/numbers and boolean-ops).

use crate::ast::{BinaryOp, UnaryOp};
use crate::error::RuntimeError;
use crate::value::{Object, Value};

/// Numeric rank used for promotion: Integer < LongInteger < Float < Double.
fn rank(value: &Value) -> Option<u8> {
    match value {
        Value::Integer(_) => Some(0),
        Value::LongInteger(_) => Some(1),
        Value::Float(_) => Some(2),
        Value::Double(_) => Some(3),
        _ => None,
    }
}

fn mismatch(op: BinaryOp, left: &Value, right: &Value) -> RuntimeError {
    RuntimeError::type_mismatch_operator(op.symbol(), left, right)
}

/// Boxed operands (roInt, roString...) behave like their intrinsic values.
pub fn binary(op: BinaryOp, left: &Value, right: &Value) -> Result<Value, RuntimeError> {
    if left.is_boxed() || right.is_boxed() {
        return binary_intrinsic(op, &left.unboxed(), &right.unboxed());
    }
    binary_intrinsic(op, left, right)
}

fn binary_intrinsic(op: BinaryOp, left: &Value, right: &Value) -> Result<Value, RuntimeError> {
    use BinaryOp::*;
    match op {
        Equal | NotEqual | Less | LessEqual | Greater | GreaterEqual => compare(op, left, right),
        Add if matches!((left, right), (Value::String(_), Value::String(_))) => {
            let (Value::String(a), Value::String(b)) = (left, right) else {
                unreachable!()
            };
            let mut text = String::with_capacity(a.len() + b.len());
            text.push_str(a);
            text.push_str(b);
            Ok(Value::string(text))
        }
        Add | Subtract | Multiply => arithmetic(op, left, right),
        Divide => divide(left, right),
        IntegerDivide => integer_divide(left, right),
        Modulo => modulo(left, right),
        Power => power(left, right),
        LeftShift | RightShift => shift(op, left, right),
        And | Or => bitwise(op, left, right),
    }
}

fn arithmetic(op: BinaryOp, left: &Value, right: &Value) -> Result<Value, RuntimeError> {
    let (Some(a), Some(b)) = (rank(left), rank(right)) else {
        return Err(mismatch(op, left, right));
    };
    let f = |x: f64, y: f64| match op {
        BinaryOp::Add => x + y,
        BinaryOp::Subtract => x - y,
        _ => x * y,
    };
    Ok(match a.max(b) {
        0 => {
            let (Value::Integer(x), Value::Integer(y)) = (left, right) else {
                unreachable!()
            };
            let result = match op {
                BinaryOp::Add => x.checked_add(*y),
                BinaryOp::Subtract => x.checked_sub(*y),
                _ => x.checked_mul(*y),
            };
            // Device: 2147483647 + 1 prints 2147483648 (promoted, not wrapped).
            result.map_or_else(
                || Value::Double(f(f64::from(*x), f64::from(*y))),
                Value::Integer,
            )
        }
        1 => {
            let (x, y) = (left.as_i64().unwrap_or(0), right.as_i64().unwrap_or(0));
            Value::LongInteger(match op {
                BinaryOp::Add => x.wrapping_add(y),
                BinaryOp::Subtract => x.wrapping_sub(y),
                _ => x.wrapping_mul(y),
            })
        }
        2 => Value::Float(f(left.as_f64().unwrap_or(0.0), right.as_f64().unwrap_or(0.0)) as f32),
        _ => Value::Double(f(
            left.as_f64().unwrap_or(0.0),
            right.as_f64().unwrap_or(0.0),
        )),
    })
}

fn float_result(left: &Value, right: &Value, value: f64) -> Value {
    if matches!(left, Value::Double(_)) || matches!(right, Value::Double(_)) {
        Value::Double(value)
    } else {
        Value::Float(value as f32)
    }
}

fn divide(left: &Value, right: &Value) -> Result<Value, RuntimeError> {
    let (Some(x), Some(y)) = (left.as_f64(), right.as_f64()) else {
        return Err(mismatch(BinaryOp::Divide, left, right));
    };
    if matches!(left, Value::LongInteger(_)) || matches!(right, Value::LongInteger(_)) {
        return Ok(Value::Double(x / y));
    }
    // Integer / Integer yields Float (7 / 2 = 3.5).
    Ok(float_result(left, right, x / y))
}

fn integer_divide(left: &Value, right: &Value) -> Result<Value, RuntimeError> {
    let (Some(x), Some(y)) = (left.as_i64(), right.as_i64()) else {
        return Err(mismatch(BinaryOp::IntegerDivide, left, right));
    };
    if y == 0 {
        return Err(RuntimeError::divide_by_zero());
    }
    let quotient = x.wrapping_div(y); // truncates toward zero: -7 \ 2 = -3
    Ok(
        if matches!(left, Value::LongInteger(_)) || matches!(right, Value::LongInteger(_)) {
            Value::LongInteger(quotient)
        } else {
            Value::Integer(quotient as i32)
        },
    )
}

fn modulo(left: &Value, right: &Value) -> Result<Value, RuntimeError> {
    let (Some(a), Some(b)) = (rank(left), rank(right)) else {
        return Err(mismatch(BinaryOp::Modulo, left, right));
    };
    if a.max(b) >= 2 {
        let (x, y) = (left.as_f64().unwrap_or(0.0), right.as_f64().unwrap_or(0.0));
        return Ok(float_result(left, right, x % y));
    }
    let (x, y) = (left.as_i64().unwrap_or(0), right.as_i64().unwrap_or(0));
    if y == 0 {
        return Err(RuntimeError::divide_by_zero());
    }
    // Remainder takes the sign of the dividend: -7 mod 3 = -1.
    Ok(if a.max(b) == 1 {
        Value::LongInteger(x.wrapping_rem(y))
    } else {
        Value::Integer(x.wrapping_rem(y) as i32)
    })
}

fn power(left: &Value, right: &Value) -> Result<Value, RuntimeError> {
    let (Some(x), Some(y)) = (left.as_f64(), right.as_f64()) else {
        return Err(mismatch(BinaryOp::Power, left, right));
    };
    // Device: type(2 ^ 2) = Integer, 10 ^ -2 = 0.01 (Float).
    if let (Value::Integer(base), Value::Integer(exponent)) = (left, right) {
        if *exponent >= 0 {
            if let Some(result) = base.checked_pow(*exponent as u32) {
                return Ok(Value::Integer(result));
            }
        }
    }
    Ok(float_result(left, right, x.powf(y)))
}

fn shift(op: BinaryOp, left: &Value, right: &Value) -> Result<Value, RuntimeError> {
    let amount = match right {
        Value::Integer(v) => *v as u32,
        Value::LongInteger(v) => *v as u32,
        _ => return Err(mismatch(op, left, right)),
    };
    match left {
        Value::Integer(v) => Ok(Value::Integer(if op == BinaryOp::LeftShift {
            v.wrapping_shl(amount)
        } else {
            v.wrapping_shr(amount)
        })),
        Value::LongInteger(v) => Ok(Value::LongInteger(if op == BinaryOp::LeftShift {
            v.wrapping_shl(amount)
        } else {
            v.wrapping_shr(amount)
        })),
        _ => Err(mismatch(op, left, right)),
    }
}

/// AND/OR on integers are bitwise (12 and 10 = 8). Booleans are handled by the
/// short-circuit instructions before reaching here.
pub fn bitwise(op: BinaryOp, left: &Value, right: &Value) -> Result<Value, RuntimeError> {
    if left.is_boxed() || right.is_boxed() {
        return bitwise(op, &left.unboxed(), &right.unboxed());
    }
    match (left, right) {
        (Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(if op == BinaryOp::And {
            *a && *b
        } else {
            *a || *b
        })),
        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(if op == BinaryOp::And {
            a & b
        } else {
            a | b
        })),
        _ if matches!(left, Value::Integer(_) | Value::LongInteger(_))
            && matches!(right, Value::Integer(_) | Value::LongInteger(_)) =>
        {
            let (a, b) = (left.as_i64().unwrap_or(0), right.as_i64().unwrap_or(0));
            Ok(Value::LongInteger(if op == BinaryOp::And {
                a & b
            } else {
                a | b
            }))
        }
        _ => Err(mismatch(op, left, right)),
    }
}

fn compare(op: BinaryOp, left: &Value, right: &Value) -> Result<Value, RuntimeError> {
    use std::cmp::Ordering;
    use BinaryOp::*;
    let ordering: Option<Ordering> = match (left, right) {
        _ if left.is_numeric() && right.is_numeric() => match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => Some(a.cmp(b)),
            (
                Value::LongInteger(_) | Value::Integer(_),
                Value::LongInteger(_) | Value::Integer(_),
            ) => Some(left.as_i64().cmp(&right.as_i64())),
            _ => left
                .as_f64()
                .unwrap_or(0.0)
                .partial_cmp(&right.as_f64().unwrap_or(0.0)),
        },
        (Value::String(a), Value::String(b)) => Some(a.as_bytes().cmp(b.as_bytes())),
        (Value::Boolean(a), Value::Boolean(b)) if matches!(op, Equal | NotEqual) => Some(a.cmp(b)),
        (Value::Invalid, Value::Invalid) if matches!(op, Equal | NotEqual) => Some(Ordering::Equal),
        // Comparing invalid with anything (e.g. a[10] = invalid,
        // IsRIDADisabled() <> invalid) is allowed and never equal.
        (Value::Invalid, _) | (_, Value::Invalid) if matches!(op, Equal | NotEqual) => None,
        (Value::Object(a), Value::Object(b)) if matches!(op, Equal | NotEqual) => {
            if std::rc::Rc::ptr_eq(a, b) {
                Some(Ordering::Equal)
            } else {
                None
            }
        }
        _ => return Err(mismatch(op, left, right)),
    };
    let result = match (op, ordering) {
        (Equal, Some(o)) => o == Ordering::Equal,
        (Equal, None) => false,
        (NotEqual, Some(o)) => o != Ordering::Equal,
        (NotEqual, None) => true,
        (Less, Some(o)) => o == Ordering::Less,
        (LessEqual, Some(o)) => o != Ordering::Greater,
        (Greater, Some(o)) => o == Ordering::Greater,
        (GreaterEqual, Some(o)) => o != Ordering::Less,
        _ => false,
    };
    Ok(Value::Boolean(result))
}

pub fn unary(op: UnaryOp, operand: &Value) -> Result<Value, RuntimeError> {
    if operand.is_boxed() {
        return unary(op, &operand.unboxed());
    }
    match (op, operand) {
        (UnaryOp::Negate, Value::Integer(v)) => Ok(v
            .checked_neg()
            .map_or(Value::Double(-f64::from(*v)), Value::Integer)),
        (UnaryOp::Negate, Value::LongInteger(v)) => Ok(Value::LongInteger(v.wrapping_neg())),
        (UnaryOp::Negate, Value::Float(v)) => Ok(Value::Float(-v)),
        (UnaryOp::Negate, Value::Double(v)) => Ok(Value::Double(-v)),
        (UnaryOp::Plus, value) if value.is_numeric() => Ok(value.clone()),
        (UnaryOp::Not, Value::Boolean(b)) => Ok(Value::Boolean(!b)),
        // NOT on integers is bitwise: not 0 = -1.
        (UnaryOp::Not, Value::Integer(v)) => Ok(Value::Integer(!v)),
        (UnaryOp::Not, Value::LongInteger(v)) => Ok(Value::LongInteger(!v)),
        _ => {
            let symbol = match op {
                UnaryOp::Negate => "-",
                UnaryOp::Plus => "+",
                UnaryOp::Not => "NOT",
            };
            Err(RuntimeError::type_mismatch_unary(symbol, operand))
        }
    }
}

/// Truthiness for IF/WHILE conditions.
pub fn condition(value: &Value) -> Result<bool, RuntimeError> {
    if value.is_boxed() {
        return condition(&value.unboxed());
    }
    match value {
        Value::Boolean(b) => Ok(*b),
        Value::Integer(v) => Ok(*v != 0),
        Value::LongInteger(v) => Ok(*v != 0),
        Value::Float(v) => Ok(*v != 0.0),
        Value::Double(v) => Ok(*v != 0.0),
        other => Err(RuntimeError::type_mismatch(format!(
            "Type Mismatch. Unable to cast \"{}\" to \"Boolean\".",
            other.type_name()
        ))),
    }
}

/// Snapshot of the items a FOR EACH loop visits.
pub fn iteration_items(value: &Value) -> Result<Vec<Value>, RuntimeError> {
    match value {
        Value::Object(object) => match &*object.borrow() {
            Object::Array(items) | Object::List(items) | Object::XmlList(items) => {
                Ok(items.clone())
            }
            Object::ByteArray(bytes) => Ok(bytes
                .iter()
                .map(|b| Value::Integer(i32::from(*b)))
                .collect()),
            Object::AssocArray(aa) => Ok(aa
                .enumeration_keys()
                .into_iter()
                .map(Value::string)
                .collect()),
            _ => Err(RuntimeError::type_mismatch(
                "Type Mismatch. FOR EACH: object is not enumerable.".into(),
            )),
        },
        _ => Err(RuntimeError::type_mismatch(format!(
            "Type Mismatch. FOR EACH: \"{}\" is not enumerable.",
            value.type_name()
        ))),
    }
}
