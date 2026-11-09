//! Small XML parser shared by roXMLElement and the SceneGraph component
//! loader. Supports elements, attributes, text, CDATA, comments, processing
//! instructions and the predefined/numeric entities; no DTDs.

use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub struct XmlElement {
    pub name: String,
    /// Attributes in document order.
    pub attributes: Vec<(String, String)>,
    pub children: Vec<XmlNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum XmlNode {
    Element(Rc<XmlElement>),
    Text(String),
}

impl XmlElement {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Attribute lookup ignoring case (SceneGraph attributes are case-insensitive).
    pub fn attribute_ci(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn elements(&self) -> impl Iterator<Item = &Rc<XmlElement>> {
        self.children.iter().filter_map(|child| match child {
            XmlNode::Element(element) => Some(element),
            XmlNode::Text(_) => None,
        })
    }

    pub fn first_element(&self, name: &str) -> Option<&Rc<XmlElement>> {
        self.elements().find(|e| e.name.eq_ignore_ascii_case(name))
    }

    /// Concatenated direct text children.
    pub fn text(&self) -> String {
        self.children
            .iter()
            .filter_map(|child| match child {
                XmlNode::Text(text) => Some(text.as_str()),
                XmlNode::Element(_) => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct XmlError {
    pub message: String,
    pub line: u32,
}

pub fn parse(text: &str) -> Result<Rc<XmlElement>, XmlError> {
    let mut parser = Parser {
        chars: text.chars().collect(),
        position: 0,
    };
    parser.skip_misc()?;
    let root = parser.element()?;
    parser.skip_misc()?;
    if parser.position < parser.chars.len() {
        return Err(parser.error("Junk after document element"));
    }
    Ok(root)
}

struct Parser {
    chars: Vec<char>,
    position: usize,
}

impl Parser {
    fn error(&self, message: &str) -> XmlError {
        let line = self.chars[..self.position.min(self.chars.len())]
            .iter()
            .filter(|c| **c == '\n')
            .count() as u32
            + 1;
        XmlError {
            message: message.to_string(),
            line,
        }
    }

    fn starts_with(&self, prefix: &str) -> bool {
        prefix
            .chars()
            .enumerate()
            .all(|(offset, expected)| self.chars.get(self.position + offset) == Some(&expected))
    }

    fn skip_until(&mut self, terminator: &str) -> Result<String, XmlError> {
        let start = self.position;
        while self.position < self.chars.len() {
            if self.starts_with(terminator) {
                let content: String = self.chars[start..self.position].iter().collect();
                self.position += terminator.chars().count();
                return Ok(content);
            }
            self.position += 1;
        }
        Err(self.error(&format!("Missing '{terminator}'")))
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.chars.get(self.position), Some(c) if c.is_whitespace()) {
            self.position += 1;
        }
    }

    /// Whitespace, comments, processing instructions and DOCTYPE outside the root.
    fn skip_misc(&mut self) -> Result<(), XmlError> {
        loop {
            self.skip_whitespace();
            if self.starts_with("<?") {
                self.skip_until("?>")?;
            } else if self.starts_with("<!--") {
                self.skip_until("-->")?;
            } else if self.starts_with("<!") {
                self.skip_until(">")?;
            } else {
                return Ok(());
            }
        }
    }

    fn name(&mut self) -> Result<String, XmlError> {
        let start = self.position;
        while matches!(self.chars.get(self.position), Some(c) if c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
        {
            self.position += 1;
        }
        if start == self.position {
            return Err(self.error("Expected a name"));
        }
        Ok(self.chars[start..self.position].iter().collect())
    }

    fn element(&mut self) -> Result<Rc<XmlElement>, XmlError> {
        if self.chars.get(self.position) != Some(&'<') {
            return Err(self.error("Expected '<'"));
        }
        self.position += 1;
        let name = self.name()?;
        let mut attributes = Vec::new();
        loop {
            self.skip_whitespace();
            match self.chars.get(self.position) {
                Some('/') if self.starts_with("/>") => {
                    self.position += 2;
                    return Ok(Rc::new(XmlElement {
                        name,
                        attributes,
                        children: Vec::new(),
                    }));
                }
                Some('>') => {
                    self.position += 1;
                    break;
                }
                Some(_) => {
                    let key = self.name()?;
                    self.skip_whitespace();
                    if self.chars.get(self.position) != Some(&'=') {
                        return Err(self.error("Expected '='"));
                    }
                    self.position += 1;
                    self.skip_whitespace();
                    let quote = match self.chars.get(self.position) {
                        Some(q @ ('"' | '\'')) => *q,
                        _ => return Err(self.error("Expected a quoted attribute value")),
                    };
                    self.position += 1;
                    let raw = self.skip_until(&quote.to_string())?;
                    attributes.push((key, decode_entities(&raw)));
                }
                None => return Err(self.error("Unexpected end of document")),
            }
        }

        let mut children = Vec::new();
        let mut text = String::new();
        loop {
            if self.position >= self.chars.len() {
                return Err(self.error(&format!("Missing end tag </{name}>")));
            }
            if self.starts_with("</") {
                self.position += 2;
                let closing = self.name()?;
                if closing != name {
                    return Err(self.error(&format!("Mismatched end tag </{closing}>")));
                }
                self.skip_whitespace();
                if self.chars.get(self.position) != Some(&'>') {
                    return Err(self.error("Expected '>'"));
                }
                self.position += 1;
                push_text(&mut children, &mut text);
                return Ok(Rc::new(XmlElement {
                    name,
                    attributes,
                    children,
                }));
            } else if self.starts_with("<!--") {
                self.position += 4;
                self.skip_until("-->")?;
            } else if self.starts_with("<![CDATA[") {
                self.position += 9;
                text.push_str(&self.skip_until("]]>")?);
            } else if self.starts_with("<?") {
                self.skip_until("?>")?;
            } else if self.chars[self.position] == '<' {
                push_text(&mut children, &mut text);
                children.push(XmlNode::Element(self.element()?));
            } else {
                let start = self.position;
                while self.position < self.chars.len() && self.chars[self.position] != '<' {
                    self.position += 1;
                }
                let raw: String = self.chars[start..self.position].iter().collect();
                text.push_str(&decode_entities(&raw));
            }
        }
    }
}

/// Text nodes made only of whitespace (indentation) are dropped.
fn push_text(children: &mut Vec<XmlNode>, text: &mut String) {
    if !text.trim().is_empty() {
        children.push(XmlNode::Text(std::mem::take(text)));
    } else {
        text.clear();
    }
}

pub fn decode_entities(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let Some(end) = after.find(';') else {
            out.push_str(after);
            return out;
        };
        let entity = &after[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => entity[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_elements_attributes_and_entities() {
        let root = parse(
            "<?xml version=\"1.0\"?>\n<rss version=\"2.0\"><!-- c --><item id='1'>B &amp; C</item><x/></rss>",
        )
        .unwrap();
        assert_eq!(root.name, "rss");
        assert_eq!(root.attribute("version"), Some("2.0"));
        let item = root.first_element("item").unwrap();
        assert_eq!(item.text(), "B & C");
        assert_eq!(root.elements().count(), 2);
        assert!(parse("<open>").is_err());
    }
}
