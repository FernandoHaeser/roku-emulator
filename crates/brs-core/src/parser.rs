//! Recursive-descent parser: tokens -> AST.

/// Hidden method name the `@` operator compiles to (see builtins).
pub const ATTRIBUTE_METHOD: &str = "$attr";

use std::fmt;
use std::rc::Rc;

use crate::ast::*;
use crate::lexer::{tokenize, Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub line: u32,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (line {})", self.message, self.line)
    }
}

type ParseResult<T> = Result<T, ParseError>;

/// Parses a whole `.brs` file into its function declarations.
pub fn parse_source(source: &str) -> ParseResult<Vec<FunctionDecl>> {
    let tokens = tokenize(source).map_err(|e| ParseError {
        message: e.message,
        line: e.line,
    })?;
    Parser { tokens, pos: 0 }.program()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

/// Which keyword sequence closes the block currently being parsed.
#[derive(Clone, Copy)]
enum BlockEnd {
    Function,
    If,
    For,
    While,
    Try,
}

impl Parser {
    // ---- token helpers -------------------------------------------------

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_at(&self, offset: usize) -> &TokenKind {
        let index = (self.pos + offset).min(self.tokens.len() - 1);
        &self.tokens[index].kind
    }

    fn line(&self) -> u32 {
        self.tokens[self.pos].line
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        token
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.peek() == kind
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn error<T>(&self, message: impl Into<String>) -> ParseResult<T> {
        Err(ParseError {
            message: message.into(),
            line: self.line(),
        })
    }

    fn expect(&mut self, kind: &TokenKind, what: &str) -> ParseResult<Token> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            self.error(format!("Syntax Error: expected {what}"))
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), TokenKind::Newline | TokenKind::Colon) {
            self.advance();
        }
    }

    fn skip_line_breaks(&mut self) {
        while self.check(&TokenKind::Newline) {
            self.advance();
        }
    }

    fn at_statement_end(&self) -> bool {
        matches!(
            self.peek(),
            TokenKind::Newline | TokenKind::Colon | TokenKind::Eof | TokenKind::Else
        )
    }

    /// Identifiers, and keywords used as names (allowed after `.` and as AA keys).
    fn name_token(&mut self) -> ParseResult<String> {
        let token = self.tokens[self.pos].clone();
        let is_name = token.kind == TokenKind::Identifier
            || (token
                .text
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
                && !matches!(token.kind, TokenKind::String(_)));
        if is_name {
            self.advance();
            Ok(token.text)
        } else {
            self.error("Syntax Error: expected a name")
        }
    }

    // ---- program and functions -----------------------------------------

    fn program(&mut self) -> ParseResult<Vec<FunctionDecl>> {
        let mut functions = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                TokenKind::Eof => return Ok(functions),
                TokenKind::Function | TokenKind::Sub => functions.push(self.function_decl(true)?),
                TokenKind::Identifier
                    if self.tokens[self.pos].text.eq_ignore_ascii_case("library") =>
                {
                    // `Library "v30/bslCore.brs"`: system libraries are not emulated yet.
                    self.advance();
                    self.advance();
                }
                _ => return self.error("Syntax Error: expected function or sub"),
            }
        }
    }

    fn function_decl(&mut self, named: bool) -> ParseResult<FunctionDecl> {
        let line = self.line();
        let is_sub = self.advance().kind == TokenKind::Sub;
        let name = if named {
            Some(self.name_token()?)
        } else {
            None
        };
        self.expect(&TokenKind::LeftParen, "(")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let param_name = self.name_token()?;
                let default = if self.eat(&TokenKind::Equal) {
                    Some(self.expression()?)
                } else {
                    None
                };
                let ty = if self.eat(&TokenKind::As) {
                    Some(self.type_name()?)
                } else {
                    None
                };
                params.push(Param {
                    name: param_name,
                    default,
                    ty,
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RightParen, ")")?;
        let return_type = if self.eat(&TokenKind::As) {
            Some(self.type_name()?)
        } else {
            None
        };
        let body = self.block(BlockEnd::Function)?;
        let end_line = self.line();
        self.consume_block_end(BlockEnd::Function)?;
        Ok(FunctionDecl {
            name,
            params,
            return_type,
            is_sub,
            body,
            line,
            end_line,
        })
    }

    fn type_name(&mut self) -> ParseResult<TypeName> {
        let token = self.advance();
        // `as function` / `as object`: some type names are keywords.
        TypeName::parse(&token.text).map_or_else(
            || self.error(format!("Syntax Error: unknown type {}", token.text)),
            Ok,
        )
    }

    // ---- blocks ----------------------------------------------------------

    fn is_block_end(&self, end: BlockEnd) -> bool {
        use TokenKind::*;
        let next = self.peek_at(1);
        match end {
            BlockEnd::Function => {
                matches!(self.peek(), EndFunction | EndSub)
                    || (self.check(&End) && matches!(next, Function | Sub))
            }
            BlockEnd::If => {
                matches!(self.peek(), Else | ElseIf | EndIf) || (self.check(&End) && *next == If)
            }
            BlockEnd::For => {
                matches!(self.peek(), EndFor | Next) || (self.check(&End) && *next == For)
            }
            BlockEnd::While => self.check(&EndWhile) || (self.check(&End) && *next == While),
            BlockEnd::Try => {
                matches!(self.peek(), Catch | EndTry) || (self.check(&End) && *next == Try)
            }
        }
    }

    fn consume_block_end(&mut self, end: BlockEnd) -> ParseResult<()> {
        use TokenKind::*;
        let single = match end {
            BlockEnd::Function => matches!(self.peek(), EndFunction | EndSub),
            BlockEnd::If => self.check(&EndIf),
            BlockEnd::For => matches!(self.peek(), EndFor | Next),
            BlockEnd::While => self.check(&EndWhile),
            BlockEnd::Try => self.check(&EndTry),
        };
        if single {
            let was_next = self.check(&Next);
            self.advance();
            // `next i` may repeat the loop variable.
            if was_next && self.check(&Identifier) {
                self.advance();
            }
            return Ok(());
        }
        if self.check(&End) {
            self.advance();
            self.advance();
            return Ok(());
        }
        self.error("Syntax Error: missing block terminator")
    }

    fn block(&mut self, end: BlockEnd) -> ParseResult<Vec<Stmt>> {
        let mut statements = Vec::new();
        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Eof) {
                return self.error("Syntax Error: unexpected end of file");
            }
            if self.is_block_end(end) {
                return Ok(statements);
            }
            statements.push(self.statement()?);
            if !self.at_statement_end() {
                return self.error("Syntax Error: expected end of statement");
            }
        }
    }

    // ---- statements ------------------------------------------------------

    fn statement(&mut self) -> ParseResult<Stmt> {
        use TokenKind::*;
        let line = self.line();
        let kind = match self.peek().clone() {
            Print | Question => {
                self.advance();
                StmtKind::Print(self.print_items()?)
            }
            If => return self.if_statement(),
            For if *self.peek_at(1) == Each => self.for_each()?,
            For => self.for_loop()?,
            While => {
                self.advance();
                let condition = self.expression()?;
                let body = self.block(BlockEnd::While)?;
                self.consume_block_end(BlockEnd::While)?;
                StmtKind::While { condition, body }
            }
            ExitFor => {
                self.advance();
                StmtKind::ExitFor
            }
            ExitWhile => {
                self.advance();
                StmtKind::ExitWhile
            }
            Exit => {
                self.advance();
                match self.advance().kind {
                    For => StmtKind::ExitFor,
                    While => StmtKind::ExitWhile,
                    _ => return self.error("Syntax Error: expected FOR or WHILE after EXIT"),
                }
            }
            Continue => {
                self.advance();
                match self.advance().kind {
                    For => StmtKind::ContinueFor,
                    While => StmtKind::ContinueWhile,
                    _ => return self.error("Syntax Error: expected FOR or WHILE after CONTINUE"),
                }
            }
            Return => {
                self.advance();
                StmtKind::Return(if self.at_statement_end() {
                    None
                } else {
                    Some(self.expression()?)
                })
            }
            Dim => {
                self.advance();
                let name = self.name_token()?;
                self.expect(&LeftBracket, "[")?;
                let mut dimensions = vec![self.expression()?];
                while self.eat(&Comma) {
                    dimensions.push(self.expression()?);
                }
                self.expect(&RightBracket, "]")?;
                StmtKind::Dim { name, dimensions }
            }
            Try => {
                self.advance();
                let body = self.block(BlockEnd::Try)?;
                self.expect(&Catch, "CATCH")?;
                let error_variable = self.name_token()?;
                let handler = self.block(BlockEnd::Try)?;
                self.consume_block_end(BlockEnd::Try)?;
                StmtKind::Try {
                    body,
                    error_variable,
                    handler,
                }
            }
            Throw => {
                self.advance();
                StmtKind::Throw(self.expression()?)
            }
            Stop => {
                self.advance();
                StmtKind::Stop
            }
            End => {
                self.advance();
                StmtKind::End
            }
            Goto => return self.error("Syntax Error: GOTO is not supported yet"),
            _ => self.assignment_or_call()?,
        };
        Ok(Stmt { kind, line })
    }

    fn print_items(&mut self) -> ParseResult<Vec<PrintItem>> {
        let mut items = Vec::new();
        while !self.at_statement_end() {
            if self.eat(&TokenKind::Comma) {
                items.push(PrintItem::Comma);
            } else if self.eat(&TokenKind::Semicolon) {
                items.push(PrintItem::Semicolon);
            } else {
                items.push(PrintItem::Expr(self.expression()?));
            }
        }
        Ok(items)
    }

    fn assignment_or_call(&mut self) -> ParseResult<StmtKind> {
        use TokenKind::*;
        let target = self.postfix()?;
        let compound = match self.peek() {
            Equal => Some(None),
            PlusEqual => Some(Some(BinaryOp::Add)),
            MinusEqual => Some(Some(BinaryOp::Subtract)),
            StarEqual => Some(Some(BinaryOp::Multiply)),
            SlashEqual => Some(Some(BinaryOp::Divide)),
            BackslashEqual => Some(Some(BinaryOp::IntegerDivide)),
            LeftShiftEqual => Some(Some(BinaryOp::LeftShift)),
            RightShiftEqual => Some(Some(BinaryOp::RightShift)),
            _ => None,
        };
        if let Some(op) = compound {
            self.advance();
            let value = self.expression()?;
            return Ok(StmtKind::Assign {
                target: self.to_lvalue(target)?,
                op,
                value,
            });
        }
        if matches!(self.peek(), PlusPlus | MinusMinus) {
            let delta = if self.advance().kind == PlusPlus {
                1
            } else {
                -1
            };
            return Ok(StmtKind::Increment {
                target: self.to_lvalue(target)?,
                delta,
            });
        }
        match target {
            Expr::Call(..) => Ok(StmtKind::Expression(target)),
            _ => self.error("Syntax Error: expected assignment or function call"),
        }
    }

    fn to_lvalue(&self, expr: Expr) -> ParseResult<LValue> {
        match expr {
            Expr::Identifier(name) => Ok(LValue::Variable(name)),
            Expr::Dot(object, name) => Ok(LValue::Dot(*object, name)),
            Expr::Index(object, indexes) => Ok(LValue::Index(*object, indexes)),
            _ => self.error("Syntax Error: invalid assignment target"),
        }
    }

    fn if_statement(&mut self) -> ParseResult<Stmt> {
        let line = self.line();
        self.advance(); // if
        let condition = self.expression()?;
        let has_then = self.eat(&TokenKind::Then);
        if self.check(&TokenKind::Newline) || self.check(&TokenKind::Eof) {
            return self.block_if(condition, line);
        }
        if !has_then && self.check(&TokenKind::Colon) {
            return self.block_if(condition, line);
        }
        // Single-line form: IF cond THEN stmt[: stmt]* [ELSE stmt[: stmt]*]
        let then_branch = self.inline_statements()?;
        let otherwise = if self.eat(&TokenKind::Else) {
            if self.check(&TokenKind::If) {
                Some(vec![self.if_statement()?])
            } else {
                Some(self.inline_statements()?)
            }
        } else {
            None
        };
        Ok(Stmt {
            kind: StmtKind::If {
                branches: vec![(condition, then_branch)],
                otherwise,
            },
            line,
        })
    }

    fn inline_statements(&mut self) -> ParseResult<Vec<Stmt>> {
        let mut statements = vec![self.statement()?];
        while self.check(&TokenKind::Colon)
            && !matches!(self.peek_at(1), TokenKind::Newline | TokenKind::Eof)
        {
            self.advance();
            statements.push(self.statement()?);
        }
        Ok(statements)
    }

    fn block_if(&mut self, condition: Expr, line: u32) -> ParseResult<Stmt> {
        let mut branches = vec![(condition, self.block(BlockEnd::If)?)];
        let mut otherwise = None;
        loop {
            let is_else_if = self.check(&TokenKind::ElseIf)
                || (self.check(&TokenKind::Else) && *self.peek_at(1) == TokenKind::If);
            if is_else_if {
                if self.advance().kind == TokenKind::Else {
                    self.advance();
                }
                let condition = self.expression()?;
                self.eat(&TokenKind::Then);
                branches.push((condition, self.block(BlockEnd::If)?));
            } else if self.eat(&TokenKind::Else) {
                otherwise = Some(self.block(BlockEnd::If)?);
            } else {
                self.consume_block_end(BlockEnd::If)?;
                return Ok(Stmt {
                    kind: StmtKind::If {
                        branches,
                        otherwise,
                    },
                    line,
                });
            }
        }
    }

    fn for_loop(&mut self) -> ParseResult<StmtKind> {
        self.advance(); // for
        let variable = self.name_token()?;
        self.expect(&TokenKind::Equal, "=")?;
        let start = self.expression()?;
        self.expect(&TokenKind::To, "TO")?;
        let end = self.expression()?;
        let step = if self.eat(&TokenKind::Step) {
            Some(self.expression()?)
        } else {
            None
        };
        let body = self.block(BlockEnd::For)?;
        self.consume_block_end(BlockEnd::For)?;
        Ok(StmtKind::For {
            variable,
            start,
            end,
            step,
            body,
        })
    }

    fn for_each(&mut self) -> ParseResult<StmtKind> {
        self.advance(); // for
        self.advance(); // each
        let variable = self.name_token()?;
        self.expect(&TokenKind::In, "IN")?;
        let iterable = self.expression()?;
        let body = self.block(BlockEnd::For)?;
        self.consume_block_end(BlockEnd::For)?;
        Ok(StmtKind::ForEach {
            variable,
            iterable,
            body,
        })
    }

    // ---- expressions (lowest to highest precedence) ----------------------

    fn expression(&mut self) -> ParseResult<Expr> {
        self.or()
    }

    fn or(&mut self) -> ParseResult<Expr> {
        let mut left = self.and()?;
        while self.eat(&TokenKind::Or) {
            left = Expr::Binary(BinaryOp::Or, Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }

    fn and(&mut self) -> ParseResult<Expr> {
        let mut left = self.not()?;
        while self.eat(&TokenKind::And) {
            left = Expr::Binary(BinaryOp::And, Box::new(left), Box::new(self.not()?));
        }
        Ok(left)
    }

    /// NOT binds looser than comparisons: `not x = 3` is `not (x = 3)`.
    fn not(&mut self) -> ParseResult<Expr> {
        if self.eat(&TokenKind::Not) {
            return Ok(Expr::Unary(UnaryOp::Not, Box::new(self.not()?)));
        }
        self.comparison()
    }

    fn comparison(&mut self) -> ParseResult<Expr> {
        let mut left = self.shift()?;
        loop {
            let op = match self.peek() {
                TokenKind::Equal => BinaryOp::Equal,
                TokenKind::NotEqual => BinaryOp::NotEqual,
                TokenKind::Less => BinaryOp::Less,
                TokenKind::LessEqual => BinaryOp::LessEqual,
                TokenKind::Greater => BinaryOp::Greater,
                TokenKind::GreaterEqual => BinaryOp::GreaterEqual,
                _ => return Ok(left),
            };
            self.advance();
            left = Expr::Binary(op, Box::new(left), Box::new(self.shift()?));
        }
    }

    fn shift(&mut self) -> ParseResult<Expr> {
        let mut left = self.additive()?;
        loop {
            let op = match self.peek() {
                TokenKind::LeftShift => BinaryOp::LeftShift,
                TokenKind::RightShift => BinaryOp::RightShift,
                _ => return Ok(left),
            };
            self.advance();
            left = Expr::Binary(op, Box::new(left), Box::new(self.additive()?));
        }
    }

    fn additive(&mut self) -> ParseResult<Expr> {
        let mut left = self.multiplicative()?;
        loop {
            let op = match self.peek() {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Subtract,
                _ => return Ok(left),
            };
            self.advance();
            left = Expr::Binary(op, Box::new(left), Box::new(self.multiplicative()?));
        }
    }

    fn multiplicative(&mut self) -> ParseResult<Expr> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                TokenKind::Star => BinaryOp::Multiply,
                TokenKind::Slash => BinaryOp::Divide,
                TokenKind::Backslash => BinaryOp::IntegerDivide,
                TokenKind::Mod => BinaryOp::Modulo,
                _ => return Ok(left),
            };
            self.advance();
            left = Expr::Binary(op, Box::new(left), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> ParseResult<Expr> {
        match self.peek() {
            TokenKind::Minus => {
                self.advance();
                Ok(Expr::Unary(UnaryOp::Negate, Box::new(self.unary()?)))
            }
            TokenKind::Plus => {
                self.advance();
                Ok(Expr::Unary(UnaryOp::Plus, Box::new(self.unary()?)))
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> ParseResult<Expr> {
        let mut left = self.postfix()?;
        while self.eat(&TokenKind::Caret) {
            // The exponent may carry its own sign: 10 ^ -2.
            let exponent = match self.peek() {
                TokenKind::Minus | TokenKind::Plus => self.unary()?,
                _ => self.postfix()?,
            };
            left = Expr::Binary(BinaryOp::Power, Box::new(left), Box::new(exponent));
        }
        Ok(left)
    }

    fn postfix(&mut self) -> ParseResult<Expr> {
        let mut expr = self.primary()?;
        loop {
            match self.peek() {
                TokenKind::LeftParen => {
                    self.advance();
                    let mut args = Vec::new();
                    self.skip_line_breaks();
                    if !self.check(&TokenKind::RightParen) {
                        loop {
                            self.skip_line_breaks();
                            args.push(self.expression()?);
                            self.skip_line_breaks();
                            if !self.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(&TokenKind::RightParen, ")")?;
                    expr = Expr::Call(Box::new(expr), args);
                }
                TokenKind::LeftBracket => {
                    self.advance();
                    let mut indexes = vec![self.expression()?];
                    while self.eat(&TokenKind::Comma) {
                        indexes.push(self.expression()?);
                    }
                    self.expect(&TokenKind::RightBracket, "]")?;
                    expr = Expr::Index(Box::new(expr), indexes);
                }
                TokenKind::Question if self.peek_at(1) == &TokenKind::Dot => {
                    self.advance();
                    self.advance();
                    expr = Expr::OptionalDot(Box::new(expr), self.name_token()?);
                }
                TokenKind::Dot => {
                    self.advance();
                    let name = self.name_token()?;
                    expr = Expr::Dot(Box::new(expr), name);
                }
                TokenKind::At => {
                    // XML attribute access: `element@name`.
                    self.advance();
                    let name = self.name_token()?;
                    expr = Expr::Call(
                        Box::new(Expr::Dot(Box::new(expr), ATTRIBUTE_METHOD.to_string())),
                        vec![Expr::Literal(Literal::String(Rc::from(name)))],
                    );
                }
                _ => return Ok(expr),
            }
        }
    }

    fn primary(&mut self) -> ParseResult<Expr> {
        use TokenKind::*;
        let token = self.advance();
        let literal = |l| Ok(Expr::Literal(l));
        match token.kind {
            Integer(v) => literal(Literal::Integer(v)),
            LongInteger(v) => literal(Literal::LongInteger(v)),
            Float(v) => literal(Literal::Float(v)),
            Double(v) => literal(Literal::Double(v)),
            String(v) => literal(Literal::String(Rc::from(v))),
            True => literal(Literal::Boolean(true)),
            False => literal(Literal::Boolean(false)),
            Invalid => literal(Literal::Invalid),
            Identifier => Ok(Expr::Identifier(token.text)),
            LeftParen => {
                let inner = self.expression()?;
                self.expect(&RightParen, ")")?;
                Ok(inner)
            }
            LeftBracket => self.array_literal(),
            LeftBrace => self.assoc_array_literal(),
            Function | Sub => {
                self.pos -= 1;
                Ok(Expr::Function(Rc::new(self.function_decl(false)?)))
            }
            _ => {
                self.pos -= 1;
                self.error(format!("Syntax Error: unexpected '{}'", token.text.trim()))
            }
        }
    }

    fn array_literal(&mut self) -> ParseResult<Expr> {
        let mut items = Vec::new();
        loop {
            self.skip_line_breaks();
            if self.eat(&TokenKind::RightBracket) {
                return Ok(Expr::ArrayLiteral(items));
            }
            items.push(self.expression()?);
            let newline_separator = self.check(&TokenKind::Newline);
            self.skip_line_breaks();
            if !self.eat(&TokenKind::Comma)
                && !newline_separator
                && !self.check(&TokenKind::RightBracket)
            {
                return self.error("Syntax Error: expected , or ] in array literal");
            }
        }
    }

    fn assoc_array_literal(&mut self) -> ParseResult<Expr> {
        let mut entries = Vec::new();
        loop {
            self.skip_line_breaks();
            if self.eat(&TokenKind::RightBrace) {
                return Ok(Expr::AssocArrayLiteral(entries));
            }
            let key = match self.peek().clone() {
                // Quoted keys keep their case; bare keys are lowercased like dot access.
                TokenKind::String(text) => {
                    self.advance();
                    text
                }
                _ => self.name_token()?.to_ascii_lowercase(),
            };
            self.expect(&TokenKind::Colon, ":")?;
            self.skip_line_breaks();
            entries.push((key, self.expression()?));
            let newline_separator = self.check(&TokenKind::Newline);
            self.skip_line_breaks();
            if !self.eat(&TokenKind::Comma)
                && !newline_separator
                && !self.check(&TokenKind::RightBrace)
            {
                return self.error("Syntax Error: expected , or } in associative array literal");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(source: &str) -> Vec<Stmt> {
        let functions = parse_source(&format!("sub Main()\n{source}\nend sub\n")).unwrap();
        functions.into_iter().next().unwrap().body
    }

    #[test]
    fn assignment_is_not_comparison() {
        let statements = body("x = 1 = 2");
        assert!(matches!(
            &statements[0].kind,
            StmtKind::Assign {
                value: Expr::Binary(BinaryOp::Equal, ..),
                ..
            }
        ));
    }

    #[test]
    fn not_binds_looser_than_comparison() {
        let statements = body("print not x = 3");
        let StmtKind::Print(items) = &statements[0].kind else {
            panic!()
        };
        assert!(
            matches!(&items[0], PrintItem::Expr(Expr::Unary(UnaryOp::Not, inner)) if matches!(**inner, Expr::Binary(BinaryOp::Equal, ..)))
        );
    }

    #[test]
    fn single_line_and_block_if() {
        let statements =
            body("if x then print 1 else print 2\nif x\nprint 3\nelse if y then\nprint 4\nend if");
        assert_eq!(statements.len(), 2);
        let StmtKind::If { branches, .. } = &statements[1].kind else {
            panic!()
        };
        assert_eq!(branches.len(), 2);
    }

    #[test]
    fn loops_and_literals() {
        let statements = body("for i = 1 to 3 step 2\nprint i\nnext\nfor each x in [1,\n2]\nend for\na = {B: 1, \"C\": 2}");
        assert_eq!(statements.len(), 3);
        let StmtKind::Assign {
            value: Expr::AssocArrayLiteral(entries),
            ..
        } = &statements[2].kind
        else {
            panic!()
        };
        assert_eq!(entries[0].0, "b");
        assert_eq!(entries[1].0, "C");
    }

    #[test]
    fn multiline_collection_literals_allow_newlines_as_separators() {
        let statements = body("a = {\n first: [] ' comment\n second: { nested: true }\n third: [1\n2,\n3]\n}\nb = [\n{one: 1}\n{two: 2}\n]");
        let StmtKind::Assign {
            value: Expr::AssocArrayLiteral(entries),
            ..
        } = &statements[0].kind
        else {
            panic!()
        };
        assert_eq!(entries.len(), 3);
        let Expr::ArrayLiteral(items) = &entries[2].1 else {
            panic!()
        };
        assert_eq!(items.len(), 3);
        assert!(parse_source("sub Main()\n a = { one: 1 two: 2 }\nend sub").is_err());
        assert!(parse_source("sub Main()\n a = [1 2]\nend sub").is_err());
    }

    #[test]
    fn method_call_on_integer_literal() {
        let statements = body("print 12.toStr()");
        let StmtKind::Print(items) = &statements[0].kind else {
            panic!()
        };
        assert!(matches!(&items[0], PrintItem::Expr(Expr::Call(..))));
    }
}
