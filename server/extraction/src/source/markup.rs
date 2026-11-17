use std::ops::Range;

use super::structured::{Node, MAX_DEPTH};
use crate::models::source::{SourceLocation, SourceRecord};

const MAX_ENTITY_BYTES: usize = 9;

pub fn parse(text: &str) -> Option<(Vec<SourceRecord>, Node)> {
    let root_start = text.find('<')?;
    let mut preamble = Vec::new();
    for (index, line) in text[..root_start].lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (name, _) = line.split_once(':')?;
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
        {
            return None;
        }
        preamble.push(SourceRecord {
            location: SourceLocation::Lines {
                first: index as u32 + 1,
                last: index as u32 + 1,
            },
            text: line.trim_end().to_string(),
            fields: None,
        });
    }

    let mut open: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    let mut at = root_start;
    while at < text.len() {
        let rest = &text[at..];
        if rest.starts_with("<!DOCTYPE") || rest.starts_with("<!doctype") {
            let end = rest.find('>')?;
            at += match rest[..end].find('[') {
                Some(subset) => subset + rest[subset..].find("]>")? + "]>".len(),
                None => end + 1,
            };
        } else if let Some(skipped) = ["<?", "<!--"]
            .iter()
            .zip(["?>", "-->"])
            .find(|(start, _)| rest.starts_with(**start))
            .map(|(_, end)| rest.find(end).map(|found| found + end.len()))
        {
            at += skipped?;
        } else if let Some(data) = rest.strip_prefix("<![CDATA[") {
            let length = data.find("]]>")?;
            let end = at + "<![CDATA[".len() + length + "]]>".len();
            add_text(&mut open, data[..length].to_string(), at..end)?;
            at = end;
        } else if let Some(closing) = rest.strip_prefix("</") {
            let length = closing.find('>')?;
            let name = closing[..length].trim();
            at += 2 + length + 1;
            let depth = open.iter().rposition(|node| node.name == name)?;
            while open.len() > depth {
                let mut node = open.pop()?;
                if open.len() == depth {
                    node.span.end = at;
                }
                close(&mut open, &mut root, node);
            }
        } else if rest.starts_with('<')
            && rest[1..].starts_with(|c: char| c.is_alphabetic() || c == '_')
        {
            if root.is_some() || open.len() >= MAX_DEPTH {
                return None;
            }
            if open
                .last()
                .is_some_and(|top| top.text.is_some() && top.children.is_empty())
            {
                let leaf = open.pop()?;
                close(&mut open, &mut root, leaf);
            }
            let bytes = text.as_bytes();
            let start = at;
            at += 1;
            while !matches!(bytes.get(at)?, b'>' | b'/') && !bytes[at].is_ascii_whitespace() {
                at += 1;
            }
            let mut node = Node::new(text[start + 1..at].to_string(), start);
            let self_closing = loop {
                while bytes.get(at)?.is_ascii_whitespace() {
                    at += 1;
                }
                match bytes[at] {
                    b'>' => {
                        at += 1;
                        break false;
                    }
                    b'/' if bytes.get(at + 1) == Some(&b'>') => {
                        at += 2;
                        break true;
                    }
                    _ => {
                        while !matches!(bytes.get(at)?, b'=' | b'>' | b'/')
                            && !bytes[at].is_ascii_whitespace()
                        {
                            at += 1;
                        }
                        if bytes[at] != b'=' {
                            if bytes[at] == b'/' && bytes.get(at + 1) != Some(&b'>') {
                                at += 1;
                            }
                            continue;
                        }
                        at += 1;
                        let value_start = at;
                        let value = match bytes.get(at)? {
                            quote @ (b'"' | b'\'') => {
                                let length = text[at + 1..].find(*quote as char)?;
                                at += length + 2;
                                &text[value_start + 1..at - 1]
                            }
                            _ => {
                                while *bytes.get(at)? != b'>' && !bytes[at].is_ascii_whitespace() {
                                    at += 1;
                                }
                                &text[value_start..at]
                            }
                        };
                        node.attributes.push(unescape(value));
                    }
                }
            };
            node.head_end = at;
            node.span.end = at;
            if self_closing {
                close(&mut open, &mut root, node);
            } else {
                open.push(node);
            }
        } else {
            let first = rest.chars().next().map_or(1, char::len_utf8);
            let length = rest[first..]
                .find('<')
                .map_or(rest.len(), |found| found + first);
            let content = rest[..length].trim();
            if !content.is_empty() {
                let start = at + (length - rest[..length].trim_start().len());
                add_text(&mut open, unescape(content), start..start + content.len())?;
            }
            at += length;
        }
    }
    root.filter(|root| open.is_empty() && !root.children.is_empty())
        .map(|root| (preamble, root))
}

fn close(open: &mut [Node], root: &mut Option<Node>, node: Node) {
    match open.last_mut() {
        Some(parent) => {
            parent.span.end = parent.span.end.max(node.span.end);
            parent.children.push(node);
        }
        None => *root = Some(node),
    }
}

fn add_text(open: &mut [Node], content: String, span: Range<usize>) -> Option<()> {
    let top = open.last_mut()?;
    top.span.end = span.end;
    if top.children.is_empty() {
        match &mut top.text {
            Some(text) => text.push_str(&content),
            None => top.text = Some(content),
        }
    } else {
        let mut loose = Node::new("#text".to_string(), span.start);
        loose.span.end = span.end;
        loose.text = Some(content);
        top.children.push(loose);
    }
    Some(())
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let entity = rest[1..]
            .bytes()
            .take(MAX_ENTITY_BYTES)
            .position(|byte| byte == b';')
            .map(|end| &rest[1..end + 1])
            .and_then(|name| {
                let decoded = match name {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "quot" => '"',
                    "apos" => '\'',
                    "nbsp" => ' ',
                    _ => {
                        let code = name.strip_prefix('#')?;
                        char::from_u32(match code.strip_prefix(['x', 'X']) {
                            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                            None => code.parse().ok()?,
                        })?
                    }
                };
                Some((decoded, name.len() + 2))
            });
        match entity {
            Some((decoded, length)) => {
                out.push(decoded);
                rest = &rest[length..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
