//! Abstract syntax tree produced by the parser and consumed by the compiler.

use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    IntegerDivide,
    Modulo,
    Power,
    LeftShift,
    RightShift,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

impl BinaryOp {
    /// Spelling used in Roku error messages (`Operator "+" can't be applied ...`).
    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Subtract => "-",
            BinaryOp::Multiply => "*",
            BinaryOp::Divide => "/",
            BinaryOp::IntegerDivide => "\\",
            BinaryOp::Modulo => "MOD",
            BinaryOp::Power => "^",
            BinaryOp::LeftShift => "<<",
            BinaryOp::RightShift => ">>",
            BinaryOp::Equal => "=",
            BinaryOp::NotEqual => "<>",
            BinaryOp::Less => "<",
            BinaryOp::LessEqual => "<=",
            BinaryOp::Greater => ">",
            BinaryOp::GreaterEqual => ">=",
            BinaryOp::And => "AND",
            BinaryOp::Or => "OR",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Plus,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Invalid,
    Boolean(bool),
    Integer(i32),
    LongInteger(i64),
    Float(f32),
    Double(f64),
    String(Rc<str>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Identifier(String),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Index(Box<Expr>, Vec<Expr>),
    Dot(Box<Expr>, String),
    ArrayLiteral(Vec<Expr>),
    AssocArrayLiteral(Vec<(String, Expr)>),
    Function(Rc<FunctionDecl>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeName {
    Integer,
    LongInteger,
    Float,
    Double,
    String,
    Boolean,
    Object,
    Function,
    Dynamic,
    Void,
}

impl TypeName {
    pub fn parse(name: &str) -> Option<TypeName> {
        Some(match name.to_ascii_lowercase().as_str() {
            "integer" => TypeName::Integer,
            "longinteger" => TypeName::LongInteger,
            "float" => TypeName::Float,
            "double" => TypeName::Double,
            "string" => TypeName::String,
            "boolean" => TypeName::Boolean,
            "object" => TypeName::Object,
            "function" => TypeName::Function,
            "dynamic" => TypeName::Dynamic,
            "void" => TypeName::Void,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub default: Option<Expr>,
    pub ty: Option<TypeName>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    /// None for anonymous functions.
    pub name: Option<String>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeName>,
    pub is_sub: bool,
    pub body: Vec<Stmt>,
    pub line: u32,
    /// Line of the closing `end function` / `end sub`.
    pub end_line: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrintItem {
    Expr(Expr),
    /// `,` moves to the next 16-column zone.
    Comma,
    /// `;` concatenates without spacing.
    Semicolon,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LValue {
    Variable(String),
    Dot(Expr, String),
    Index(Expr, Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    Assign {
        target: LValue,
        op: Option<BinaryOp>,
        value: Expr,
    },
    Increment {
        target: LValue,
        delta: i32,
    },
    Expression(Expr),
    Print(Vec<PrintItem>),
    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        otherwise: Option<Vec<Stmt>>,
    },
    For {
        variable: String,
        start: Expr,
        end: Expr,
        step: Option<Expr>,
        body: Vec<Stmt>,
    },
    ForEach {
        variable: String,
        iterable: Expr,
        body: Vec<Stmt>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    ExitFor,
    ExitWhile,
    ContinueFor,
    ContinueWhile,
    Return(Option<Expr>),
    Dim {
        name: String,
        dimensions: Vec<Expr>,
    },
    Try {
        body: Vec<Stmt>,
        error_variable: String,
        handler: Vec<Stmt>,
    },
    Throw(Expr),
    Stop,
    End,
}
