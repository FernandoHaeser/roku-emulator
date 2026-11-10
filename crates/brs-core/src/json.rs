//! ParseJson / FormatJson with the device's typing and formatting rules
//! (fidelity/runtime/json): integers that fit 32 bits are Integer, larger ones
//! LongInteger, numbers with a fraction or exponent are Float; FormatJson sorts
//! keys byte-wise and escapes non-ASCII as uppercase `\uXXXX`.
//! Number formatting follows fidelity/runtime/json-numbers (see `format_number`).

use crate::value::{format_general, AssocArray, Object, Value};

pub fn parse(text: &str) -> Result<Value, String> {
    let mut parser = Parser {
        chars: text.chars().collect(),
        position: 0,
    };
    parser.skip_whitespace();
    let value = parser.value()?;
    parser.skip_whitespace();
    if parser.position < parser.chars.len() {
        return Err(parser.unknown_identifier());
    }
    Ok(value)
}

struct Parser {
    chars: Vec<char>,
    position: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.position += 1;
        }
    }

    fn unknown_identifier(&self) -> String {
        let rest: String = self.chars[self.position..].iter().collect();
        format!("Unknown identifier '{}'", rest.trim_end())
    }

    fn expect(&mut self, expected: char) -> Result<(), String> {
        self.skip_whitespace();
        if self.peek() == Some(expected) {
            self.position += 1;
            Ok(())
        } else {
            Err(format!("Expected '{expected}'"))
        }
    }

    fn keyword(&mut self, word: &str, value: Value) -> Result<Value, String> {
        let end = self.position + word.len();
        if end <= self.chars.len()
            && self.chars[self.position..end]
                .iter()
                .copied()
                .eq(word.chars())
        {
            self.position = end;
            Ok(value)
        } else {
            Err(self.unknown_identifier())
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        self.skip_whitespace();
        match self.peek() {
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Value::string(self.string()?)),
            Some('t') => self.keyword("true", Value::Boolean(true)),
            Some('f') => self.keyword("false", Value::Boolean(false)),
            Some('n') => self.keyword("null", Value::Invalid),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            Some(_) => Err(self.unknown_identifier()),
            None => Err("Unexpected end of input".into()),
        }
    }

    fn object(&mut self) -> Result<Value, String> {
        self.position += 1;
        let mut aa = AssocArray::new();
        self.skip_whitespace();
        if self.peek() == Some('}') {
            self.position += 1;
            return Ok(Value::object(Object::AssocArray(aa)));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some('"') {
                return Err("Expected a string key".into());
            }
            let key = self.string()?;
            self.expect(':')?;
            let value = self.value()?;
            aa.set(&key, value);
            self.skip_whitespace();
            match self.peek() {
                Some(',') => self.position += 1,
                Some('}') => {
                    self.position += 1;
                    return Ok(Value::object(Object::AssocArray(aa)));
                }
                _ => return Err("Expected ',' or '}'".into()),
            }
        }
    }

    fn array(&mut self) -> Result<Value, String> {
        self.position += 1;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(']') {
            self.position += 1;
            return Ok(Value::object(Object::Array(items)));
        }
        loop {
            items.push(self.value()?);
            self.skip_whitespace();
            match self.peek() {
                Some(',') => self.position += 1,
                Some(']') => {
                    self.position += 1;
                    return Ok(Value::object(Object::Array(items)));
                }
                _ => return Err("Expected ',' or ']'".into()),
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.position += 1; // opening quote
        let mut text = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err("Unterminated string".into());
            };
            self.position += 1;
            match c {
                '"' => return Ok(text),
                '\\' => {
                    let Some(escape) = self.peek() else {
                        return Err("Unterminated string".into());
                    };
                    self.position += 1;
                    match escape {
                        'n' => text.push('\n'),
                        't' => text.push('\t'),
                        'r' => text.push('\r'),
                        'b' => text.push('\u{8}'),
                        'f' => text.push('\u{c}'),
                        'u' => {
                            let high = self.hex4()?;
                            if (0xD800..0xDC00).contains(&high)
                                && self.chars.get(self.position) == Some(&'\\')
                            {
                                self.position += 2;
                                let low = self.hex4()?;
                                let code = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                                text.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                            } else {
                                text.push(char::from_u32(high).unwrap_or('\u{fffd}'));
                            }
                        }
                        other => text.push(other),
                    }
                }
                other => text.push(other),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let end = self.position + 4;
        let digits: String = self
            .chars
            .get(self.position..end)
            .ok_or("Invalid unicode escape")?
            .iter()
            .collect();
        self.position = end;
        u32::from_str_radix(&digits, 16).map_err(|_| "Invalid unicode escape".to_string())
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.position;
        let mut is_float = false;
        while let Some(c) = self.peek() {
            match c {
                '0'..='9' | '-' | '+' => {}
                '.' | 'e' | 'E' => is_float = true,
                _ => break,
            }
            self.position += 1;
        }
        let text: String = self.chars[start..self.position].iter().collect();
        if is_float {
            let value: f64 = text
                .parse()
                .map_err(|_| format!("Invalid number '{text}'"))?;
            return Ok(Value::Float(value as f32));
        }
        let value: i64 = text
            .parse()
            .map_err(|_| format!("Invalid number '{text}'"))?;
        Ok(match i32::try_from(value) {
            Ok(small) => Value::Integer(small),
            Err(_) => Value::LongInteger(value),
        })
    }
}

/// Serializes a value. Errors name the unsupported type (device diagnostic).
pub fn format(value: &Value) -> Result<String, String> {
    let mut out = String::new();
    write_value(&mut out, value, 0)?;
    Ok(out)
}

/// FormatJson does not reuse PRINT's formatting for Float/Double: integral
/// values that fit a 64-bit integer are written in full (`2147483648`, not
/// `2.14748e+09`), larger integral values use 10 significant digits and
/// fractional values use 6, like `%g`.
fn format_number(value: f64) -> String {
    const I64_LIMIT: f64 = 9_223_372_036_854_775_808.0;
    if value.fract() == 0.0 && value.abs() < I64_LIMIT {
        (value as i64).to_string()
    } else if value.fract() == 0.0 {
        format_general(value, 10)
    } else {
        format_general(value, 6)
    }
}

fn write_value(out: &mut String, value: &Value, depth: usize) -> Result<(), String> {
    if depth > 256 {
        return Err("Nested object reference".into());
    }
    match value {
        Value::Invalid => out.push_str("null"),
        Value::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Integer(v) => out.push_str(&v.to_string()),
        Value::LongInteger(v) => out.push_str(&v.to_string()),
        Value::Float(v) => out.push_str(&format_number(f64::from(*v))),
        Value::Double(v) => out.push_str(&format_number(*v)),
        Value::String(text) => write_string(out, text),
        Value::Object(object) => match &*object.borrow() {
            Object::Boxed(inner) => write_value(out, inner, depth + 1)?,
            Object::Array(items) | Object::List(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    write_value(out, item, depth + 1)?;
                }
                out.push(']');
            }
            Object::AssocArray(aa) => {
                let mut entries = aa.sorted_entries();
                entries.sort_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));
                out.push('{');
                for (index, (key, item)) in entries.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    write_string(out, key);
                    out.push(':');
                    write_value(out, item, depth + 1)?;
                }
                out.push('}');
            }
            other => return Err(format!("Value type not supported: {}", other.class_name())),
        },
        other => return Err(format!("Value type not supported: {}", other.type_name())),
    }
    Ok(())
}

fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04X}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_device_sample() {
        let value = parse(r#"{"b": 1, "a": [1, 2.5, "x", true, null], "Nested": {"k": -3}, "big": 3000000000, "e": 1e3}"#).unwrap();
        assert_eq!(
            format(&value).unwrap(),
            r#"{"Nested":{"k":-3},"a":[1,2.5,"x",true,null],"b":1,"big":3000000000,"e":1000}"#
        );
    }

    #[test]
    fn escapes_like_the_device() {
        let value = Value::string("quote \" slash / back \\ tab\t nl\n uni \u{e9}");
        assert_eq!(
            format(&value).unwrap(),
            r#""quote \" slash / back \\ tab\t nl\n uni \u00E9""#
        );
        assert_eq!(
            parse("not json").unwrap_err(),
            "Unknown identifier 'not json'"
        );
    }
}
