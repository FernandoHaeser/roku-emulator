//! BrightScript lexer. Keywords and identifiers are case-insensitive; the
//! original spelling is kept in `text` because identifiers are printed back
//! (e.g. AA keys) and error messages quote them.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Identifier,
    Integer(i32),
    LongInteger(i64),
    Float(f32),
    Double(f64),
    String(String),

    // Keywords
    And,
    As,
    Catch,
    Continue,
    Dim,
    Each,
    Else,
    ElseIf,
    End,
    EndFor,
    EndFunction,
    EndIf,
    EndSub,
    EndTry,
    EndWhile,
    Exit,
    ExitFor,
    ExitWhile,
    False,
    For,
    Function,
    Goto,
    If,
    In,
    Invalid,
    Mod,
    Next,
    Not,
    Or,
    Print,
    Return,
    Step,
    Stop,
    Sub,
    Then,
    Throw,
    To,
    True,
    Try,
    While,

    // Operators and punctuation
    Plus,
    Minus,
    Star,
    Slash,
    Backslash,
    Caret,
    LeftShift,
    RightShift,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    BackslashEqual,
    LeftShiftEqual,
    RightShiftEqual,
    PlusPlus,
    MinusMinus,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Comma,
    Semicolon,
    Colon,
    Dot,
    Question,
    At,

    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    /// Source spelling (identifiers/keywords keep their original case).
    pub text: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub line: u32,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (line {})", self.message, self.line)
    }
}

fn keyword(word: &str) -> Option<TokenKind> {
    use TokenKind::*;
    Some(match word.to_ascii_lowercase().as_str() {
        "and" => And,
        "as" => As,
        "catch" => Catch,
        "continue" => Continue,
        "dim" => Dim,
        "each" => Each,
        "else" => Else,
        "elseif" => ElseIf,
        "end" => End,
        "endfor" => EndFor,
        "endfunction" => EndFunction,
        "endif" => EndIf,
        "endsub" => EndSub,
        "endtry" => EndTry,
        "endwhile" => EndWhile,
        "exit" => Exit,
        "exitfor" => ExitFor,
        "exitwhile" => ExitWhile,
        "false" => False,
        "for" => For,
        "function" => Function,
        "goto" => Goto,
        "if" => If,
        "in" => In,
        "invalid" => Invalid,
        "mod" => Mod,
        "next" => Next,
        "not" => Not,
        "or" => Or,
        "print" => Print,
        "return" => Return,
        "step" => Step,
        "stop" => Stop,
        "sub" => Sub,
        "then" => Then,
        "throw" => Throw,
        "to" => To,
        "true" => True,
        "try" => Try,
        "while" => While,
        _ => return None,
    })
}

/// Float literals with at least this many significant digits become Double.
const DOUBLE_LITERAL_DIGITS: usize = 10;

fn significant_digits(literal: &str) -> usize {
    let mantissa = literal.split(['e', 'E']).next().unwrap_or("");
    mantissa
        .chars()
        .filter(char::is_ascii_digit)
        .skip_while(|c| *c == '0')
        .count()
}

pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    Lexer {
        chars: source.chars().collect(),
        pos: 0,
        line: 1,
        tokens: Vec::new(),
    }
    .run()
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: u32,
    tokens: Vec<Token>,
}

impl Lexer {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn push(&mut self, kind: TokenKind, text: impl Into<String>) {
        self.tokens.push(Token {
            kind,
            text: text.into(),
            line: self.line,
        });
    }

    fn error(&self, message: impl Into<String>) -> LexError {
        LexError {
            message: message.into(),
            line: self.line,
        }
    }

    fn run(mut self) -> Result<Vec<Token>, LexError> {
        while let Some(c) = self.peek() {
            match c {
                ' ' | '\t' | '\u{feff}' => self.pos += 1,
                '\r' => {
                    self.pos += 1;
                    if self.peek() == Some('\n') {
                        self.pos += 1;
                    }
                    self.newline();
                }
                '\n' => {
                    self.pos += 1;
                    self.newline();
                }
                '\'' => self.skip_comment(),
                '"' => self.string()?,
                '0'..='9' => self.number()?,
                '.' if self.peek_at(1).is_some_and(|d| d.is_ascii_digit()) => self.number()?,
                '&' if matches!(self.peek_at(1), Some('h' | 'H')) => self.hex()?,
                c if c.is_alphabetic() || c == '_' => self.word(),
                _ => self.operator()?,
            }
        }
        self.push(TokenKind::Newline, "\n");
        self.push(TokenKind::Eof, "");
        Ok(self.tokens)
    }

    fn newline(&mut self) {
        self.push(TokenKind::Newline, "\n");
        self.line += 1;
    }

    fn skip_comment(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' || c == '\r' {
                break;
            }
            self.pos += 1;
        }
    }

    fn string(&mut self) -> Result<(), LexError> {
        self.pos += 1; // opening quote
        let mut value = String::new();
        loop {
            match self.peek() {
                None | Some('\n') | Some('\r') => {
                    return Err(self.error("Unterminated string literal"))
                }
                Some('"') if self.peek_at(1) == Some('"') => {
                    value.push('"');
                    self.pos += 2;
                }
                Some('"') => {
                    self.pos += 1;
                    break;
                }
                Some(c) => {
                    value.push(c);
                    self.pos += 1;
                }
            }
        }
        let text = format!("\"{value}\"");
        self.push(TokenKind::String(value), text);
        Ok(())
    }

    fn number(&mut self) -> Result<(), LexError> {
        let start = self.pos;
        let mut is_float = false;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        if self.peek() == Some('.')
            && self
                .peek_at(1)
                .is_none_or(|c| c.is_ascii_digit() || !c.is_alphabetic())
        {
            is_float = true;
            self.pos += 1;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let mut exponent_is_double = false;
        if let Some(e) = self.peek() {
            if matches!(e, 'e' | 'E' | 'd' | 'D') {
                let sign_offset = usize::from(matches!(self.peek_at(1), Some('+' | '-')));
                if self
                    .peek_at(1 + sign_offset)
                    .is_some_and(|c| c.is_ascii_digit())
                {
                    is_float = true;
                    exponent_is_double = matches!(e, 'd' | 'D');
                    self.pos += 1 + sign_offset;
                    while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                        self.pos += 1;
                    }
                }
            }
        }
        let digits: String = self.chars[start..self.pos]
            .iter()
            .map(|c| if matches!(c, 'd' | 'D') { 'e' } else { *c })
            .collect();
        let suffix = self.peek().filter(|c| matches!(c, '%' | '!' | '#' | '&'));
        if suffix.is_some() {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        let bad = || self.error(format!("Invalid number literal {text}"));

        let kind = match suffix {
            Some('#') => TokenKind::Double(digits.parse().map_err(|_| bad())?),
            Some('!') => TokenKind::Float(digits.parse().map_err(|_| bad())?),
            Some('&') if !is_float => TokenKind::LongInteger(digits.parse().map_err(|_| bad())?),
            Some('&') => return Err(bad()),
            Some('%') if !is_float => TokenKind::Integer(digits.parse().map_err(|_| bad())?),
            Some('%') => return Err(bad()),
            _ if exponent_is_double => TokenKind::Double(digits.parse().map_err(|_| bad())?),
            // Device: 3.14159265358979 is a Double, 1234567.8 stays Float.
            // TODO(fidelity): pin the exact digit threshold with a probe.
            _ if is_float && significant_digits(&digits) >= DOUBLE_LITERAL_DIGITS => {
                TokenKind::Double(digits.parse().map_err(|_| bad())?)
            }
            _ if is_float => TokenKind::Float(digits.parse().map_err(|_| bad())?),
            _ => match digits.parse::<i32>() {
                Ok(value) => TokenKind::Integer(value),
                // Integer literals that do not fit 32 bits become Double.
                Err(_) => TokenKind::Double(digits.parse().map_err(|_| bad())?),
            },
        };
        self.push(kind, text);
        Ok(())
    }

    fn hex(&mut self) -> Result<(), LexError> {
        let start = self.pos;
        self.pos += 2; // &H
        let digits_start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
            self.pos += 1;
        }
        let digits: String = self.chars[digits_start..self.pos].iter().collect();
        if digits.is_empty() {
            return Err(self.error("Invalid hex literal"));
        }
        let long = self.peek() == Some('&');
        if long {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        let value = u64::from_str_radix(&digits, 16)
            .map_err(|_| self.error(format!("Invalid hex literal {text}")))?;
        let kind = if long {
            TokenKind::LongInteger(value as i64)
        } else {
            // Hex literals wrap into 32 bits (&HFFFFFFFF = -1).
            TokenKind::Integer(value as u32 as i32)
        };
        self.push(kind, text);
        Ok(())
    }

    fn word(&mut self) {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            self.pos += 1;
        }
        // Type designator suffixes are part of the identifier (name$, count%).
        if self
            .peek()
            .is_some_and(|c| matches!(c, '$' | '%' | '!' | '#' | '&'))
        {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        if text.eq_ignore_ascii_case("rem") {
            self.skip_comment();
            return;
        }
        let kind = keyword(&text).unwrap_or(TokenKind::Identifier);
        self.push(kind, text);
    }

    fn operator(&mut self) -> Result<(), LexError> {
        use TokenKind::*;
        let c = self.peek().unwrap_or('\0');
        let next = self.peek_at(1);
        let next2 = self.peek_at(2);
        let (kind, len) = match (c, next, next2) {
            ('<', Some('<'), Some('=')) => (LeftShiftEqual, 3),
            ('>', Some('>'), Some('=')) => (RightShiftEqual, 3),
            ('<', Some('<'), _) => (LeftShift, 2),
            ('>', Some('>'), _) => (RightShift, 2),
            ('<', Some('>'), _) => (NotEqual, 2),
            ('<', Some('='), _) => (LessEqual, 2),
            ('>', Some('='), _) => (GreaterEqual, 2),
            ('+', Some('='), _) => (PlusEqual, 2),
            ('-', Some('='), _) => (MinusEqual, 2),
            ('*', Some('='), _) => (StarEqual, 2),
            ('/', Some('='), _) => (SlashEqual, 2),
            ('\\', Some('='), _) => (BackslashEqual, 2),
            ('+', Some('+'), _) => (PlusPlus, 2),
            ('-', Some('-'), _) => (MinusMinus, 2),
            ('<', _, _) => (Less, 1),
            ('>', _, _) => (Greater, 1),
            ('=', _, _) => (Equal, 1),
            ('+', _, _) => (Plus, 1),
            ('-', _, _) => (Minus, 1),
            ('*', _, _) => (Star, 1),
            ('/', _, _) => (Slash, 1),
            ('\\', _, _) => (Backslash, 1),
            ('^', _, _) => (Caret, 1),
            ('(', _, _) => (LeftParen, 1),
            (')', _, _) => (RightParen, 1),
            ('[', _, _) => (LeftBracket, 1),
            (']', _, _) => (RightBracket, 1),
            ('{', _, _) => (LeftBrace, 1),
            ('}', _, _) => (RightBrace, 1),
            (',', _, _) => (Comma, 1),
            (';', _, _) => (Semicolon, 1),
            (':', _, _) => (Colon, 1),
            ('.', _, _) => (Dot, 1),
            ('?', _, _) => (Question, 1),
            ('@', _, _) => (At, 1),
            _ => return Err(self.error(format!("Unexpected character '{c}'"))),
        };
        let text: std::string::String = self.chars[self.pos..self.pos + len].iter().collect();
        self.pos += len;
        self.push(kind, text);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        tokenize(source)
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn numbers_and_suffixes() {
        use TokenKind::*;
        assert_eq!(
            kinds("1 1.5 2# 3& &HFF &hFFFFFFFF 1e7 3000000000"),
            vec![
                Integer(1),
                Float(1.5),
                Double(2.0),
                LongInteger(3),
                Integer(255),
                Integer(-1),
                Float(1e7),
                Double(3e9),
                Newline,
                Eof
            ]
        );
    }

    #[test]
    fn keywords_are_case_insensitive_and_rem_is_comment() {
        use TokenKind::*;
        assert_eq!(
            kinds("PRINT x ' c\nrem all\nEnd If"),
            vec![Print, Identifier, Newline, Newline, End, If, Newline, Eof]
        );
    }

    #[test]
    fn strings_with_escaped_quotes() {
        let tokens = tokenize(r#""a ""b"" c""#).unwrap();
        assert_eq!(tokens[0].kind, TokenKind::String("a \"b\" c".into()));
        assert!(tokenize("\"open").is_err());
    }
}
