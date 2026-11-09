//! Runtime errors with the codes and messages the device prints, e.g.
//! `Type Mismatch. Operator "+" can't be applied to "String" and "Integer". (runtime error &h18)`.

use std::fmt;

use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeError {
    pub code: u8,
    pub message: String,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (runtime error &h{:02x})", self.message, self.code)
    }
}

impl RuntimeError {
    pub const TYPE_MISMATCH: u8 = 0x18;
    pub const DIVIDE_BY_ZERO: u8 = 0x14;
    pub const MEMBER_NOT_FOUND: u8 = 0xf4;
    pub const NOT_A_FUNCTION: u8 = 0xe0;
    pub const WRONG_ARGUMENT_COUNT: u8 = 0xf1;
    pub const INVALID_DOT: u8 = 0xec;
    pub const INVALID_INDEX: u8 = 0xeb;
    pub const USER_THROW: u8 = 0x28;

    pub fn new(code: u8, message: impl Into<String>) -> RuntimeError {
        RuntimeError {
            code,
            message: message.into(),
        }
    }

    /// Internal: a callback already crashed the channel (dump printed);
    /// unwinds native callers without printing a second dump.
    pub fn aborted() -> RuntimeError {
        RuntimeError::new(0, "aborted")
    }

    pub fn is_aborted(&self) -> bool {
        self.code == 0
    }

    pub fn type_mismatch(message: String) -> RuntimeError {
        RuntimeError::new(Self::TYPE_MISMATCH, message)
    }

    pub fn type_mismatch_operator(symbol: &str, left: &Value, right: &Value) -> RuntimeError {
        Self::type_mismatch(format!(
            "Type Mismatch. Operator \"{symbol}\" can't be applied to \"{}\" and \"{}\".",
            left.type_name(),
            right.type_name()
        ))
    }

    pub fn type_mismatch_unary(symbol: &str, operand: &Value) -> RuntimeError {
        Self::type_mismatch(format!(
            "Type Mismatch. Operator \"{symbol}\" can't be applied to \"{}\".",
            operand.type_name()
        ))
    }

    pub fn divide_by_zero() -> RuntimeError {
        RuntimeError::new(Self::DIVIDE_BY_ZERO, "Divide by Zero.")
    }

    pub fn member_not_found() -> RuntimeError {
        RuntimeError::new(
            Self::MEMBER_NOT_FOUND,
            "Member function not found in BrightScript Component or interface.",
        )
    }

    pub fn not_a_function() -> RuntimeError {
        RuntimeError::new(
            Self::NOT_A_FUNCTION,
            "Function Call Operator ( ) attempted on non-function.",
        )
    }

    pub fn wrong_argument_count() -> RuntimeError {
        RuntimeError::new(
            Self::WRONG_ARGUMENT_COUNT,
            "Wrong number of function parameters.",
        )
    }

    pub fn invalid_dot() -> RuntimeError {
        RuntimeError::new(
            Self::INVALID_DOT,
            "'Dot' Operator attempted with invalid BrightScript Component or interface reference.",
        )
    }

    pub fn invalid_index() -> RuntimeError {
        RuntimeError::new(
            Self::INVALID_INDEX,
            "Array operation attempted on variable not DIM'd.",
        )
    }
}
