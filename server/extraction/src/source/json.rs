use serde::de::IgnoredAny;

use super::structured::{Node, MAX_DEPTH};

pub fn parse(text: &str) -> Option<Node> {
    if !text.trim_start().starts_with(['{', '[']) || nests_too_deeply(text) {
        return None;
    }
    if serde_json::from_str::<IgnoredAny>(text).is_ok() {
        return Some(value(text, &mut 0, "$", None));
    }
    let mut root = Node::new("$".to_string(), 0);
    root.is_list = true;
    let mut line_start = 0;
    for line in text.split('\n') {
        if !line.trim().is_empty() {
            if !line.trim_start().starts_with('{')
                || serde_json::from_str::<IgnoredAny>(line).is_err()
            {
                return None;
            }
            let mut at = line_start;
            root.children.push(value(text, &mut at, "$", None));
        }
        line_start += line.len() + 1;
    }
    root.span.end = text.len();
    Some(root)
}

pub fn nests_too_deeply(text: &str) -> bool {
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for byte in text.bytes() {
        match byte {
            _ if escaped => escaped = false,
            b'\\' if in_string => escaped = true,
            b'"' => in_string = !in_string,
            b'{' | b'[' if !in_string => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return true;
                }
            }
            b'}' | b']' if !in_string => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

fn value(text: &str, at: &mut usize, name: &str, key_start: Option<usize>) -> Node {
    let bytes = text.as_bytes();
    skip_whitespace(bytes, at);
    let mut node = Node::new(name.to_string(), key_start.unwrap_or(*at));
    match bytes[*at] {
        open @ (b'{' | b'[') => {
            node.is_list = open == b'[';
            *at += 1;
            loop {
                skip_whitespace(bytes, at);
                match bytes[*at] {
                    b'}' | b']' => break,
                    b',' => *at += 1,
                    _ if open == b'[' => node.children.push(value(text, at, name, None)),
                    _ => {
                        let key_start = *at;
                        let key = string(text, at);
                        skip_whitespace(bytes, at);
                        *at += 1;
                        node.children.push(value(text, at, &key, Some(key_start)));
                    }
                }
            }
            *at += 1;
        }
        b'"' => node.text = Some(string(text, at)),
        _ => {
            let start = *at;
            while !matches!(bytes.get(*at), None | Some(b',' | b']' | b'}'))
                && !bytes[*at].is_ascii_whitespace()
            {
                *at += 1;
            }
            node.text = Some(match &text[start..*at] {
                "null" => String::new(),
                literal => literal.to_string(),
            });
        }
    }
    node.span.end = *at;
    node
}

fn string(text: &str, at: &mut usize) -> String {
    let bytes = text.as_bytes();
    let start = *at;
    *at += 1;
    while bytes[*at] != b'"' {
        *at += if bytes[*at] == b'\\' { 2 } else { 1 };
    }
    *at += 1;
    serde_json::from_str(&text[start..*at]).unwrap_or_else(|_| text[start + 1..*at - 1].to_string())
}

fn skip_whitespace(bytes: &[u8], at: &mut usize) {
    while bytes.get(*at).is_some_and(u8::is_ascii_whitespace) {
        *at += 1;
    }
}
