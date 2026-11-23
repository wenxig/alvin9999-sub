use std::io;

const CODE_BLOCK: &str = "wp-block-code";

// Both WordPress pages put their import links inside the article's Gutenberg code blocks.
pub fn subscriptions(html: &str, mut emit: impl FnMut(&str) -> io::Result<()>) -> io::Result<()> {
    let article = html
        .split_once("<article")
        .and_then(|(_, rest)| rest.split_once('>'))
        .and_then(|(_, rest)| rest.split_once("</article>"))
        .map(|(article, _)| article)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing WordPress article"))?;

    let mut markup = String::new();
    let mut entities = String::new();
    let mut found = false;
    for block in article.split("<pre").skip(1) {
        let Some((attributes, rest)) = block.split_once('>') else {
            continue;
        };
        if !attributes.starts_with(char::is_whitespace) {
            continue;
        }
        if attributes != " class=\"wp-block-code\""
            && !attribute(attributes, "class").is_some_and(|classes| {
                classes
                    .split_ascii_whitespace()
                    .any(|class| class == CODE_BLOCK)
            })
        {
            continue;
        }
        let (body, _) = rest.split_once("</pre>").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "unterminated WordPress code block",
            )
        })?;
        let mut code = body.trim();
        while let Some(inner) = code
            .strip_prefix("<code>")
            .and_then(|s| s.strip_suffix("</code>"))
        {
            code = inner.trim();
        }
        let text = inline_text(code, &mut markup);
        for line in decode_entities(text, &mut entities).lines() {
            let node = line.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
            if is_subscription(node) {
                emit(node)?;
                found = true;
            }
        }
    }
    if !found {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "no subscription links in WordPress code blocks",
        ));
    }
    Ok(())
}

// WordPress emits quoted attribute values. Skip whole values to avoid matching text inside them.
fn attribute<'a>(mut attrs: &'a str, wanted: &str) -> Option<&'a str> {
    while !attrs.trim_start().is_empty() {
        attrs = attrs.trim_start();
        let end = attrs
            .find(|c: char| c.is_ascii_whitespace() || c == '=')
            .unwrap_or(attrs.len());
        let name = &attrs[..end];
        attrs = attrs[end..].trim_start();
        let Some(value) = attrs.strip_prefix('=') else {
            continue;
        };
        let value = value.trim_start();
        let (value, tail) = match value.as_bytes().first() {
            Some(&quote @ (b'"' | b'\'')) => value[1..].split_once(char::from(quote))?,
            _ => value.split_at(value.find(char::is_whitespace).unwrap_or(value.len())),
        };
        if name.eq_ignore_ascii_case(wanted) {
            return Some(value);
        }
        attrs = tail;
    }
    None
}

fn inline_text<'a>(text: &'a str, buffer: &'a mut String) -> &'a str {
    if !text.contains('<') {
        return text;
    }
    buffer.clear();
    let mut rest = text;
    while let Some((prefix, tag)) = rest.split_once('<') {
        let Some((tag, tail)) = tag.split_once('>') else {
            break;
        };
        buffer.push_str(prefix);
        rest = tail;
        if let Some((name @ ("a" | "span"), attrs)) = tag.split_once(char::is_whitespace)
            && let Some(encoded) = attribute(attrs, "data-cfemail")
            && let Some((_, after)) = tail.split_once(if name == "a" { "</a>" } else { "</span>" })
            && decode_email(encoded, buffer)
        {
            rest = after;
        }
    }
    buffer.push_str(rest);
    buffer
}

fn decode_email(encoded: &str, output: &mut String) -> bool {
    if encoded.len() < 2
        || !encoded.len().is_multiple_of(2)
        || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return false;
    }
    let key = u8::from_str_radix(&encoded[..2], 16).unwrap();
    for offset in (2..encoded.len()).step_by(2) {
        let byte = u8::from_str_radix(&encoded[offset..offset + 2], 16).unwrap();
        output.push(char::from(byte ^ key));
    }
    true
}

fn is_subscription(line: &str) -> bool {
    let Some((scheme, _)) = line.split_once("://") else {
        return false;
    };
    let [first, rest @ ..] = scheme.as_bytes() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && rest
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'.' | b'-'))
        && !scheme.eq_ignore_ascii_case("http")
        && !scheme.eq_ignore_ascii_case("https")
}

fn decode_entities<'a>(content: &'a str, buffer: &'a mut String) -> &'a str {
    buffer.clear();
    let mut cursor = 0;
    let mut candidates = content.match_indices('&').peekable();
    while let Some((start, _)) = candidates.next() {
        let end = candidates.peek().map_or(content.len(), |&(next, _)| next);
        if let Some((name, _)) = content[start + 1..end].split_once(';')
            && let Some(character) = entity(name)
        {
            buffer.push_str(&content[cursor..start]);
            buffer.push(character);
            cursor = start + name.len() + 2;
        }
    }
    if cursor == 0 {
        return content;
    }
    buffer.push_str(&content[cursor..]);
    buffer
}

fn entity(name: &str) -> Option<char> {
    if let Some(number) = name.strip_prefix('#') {
        let value = match number.strip_prefix(['x', 'X']) {
            Some(hex) if hex.bytes().all(|byte| byte.is_ascii_hexdigit()) => {
                u32::from_str_radix(hex, 16).ok()
            }
            None if number.bytes().all(|byte| byte.is_ascii_digit()) => number.parse().ok(),
            _ => None,
        }?;
        return match value {
            0xd800..=0xdfff => Some(char::REPLACEMENT_CHARACTER),
            _ => char::from_u32(value),
        };
    }
    [
        ("amp", '&'),
        ("apos", '\''),
        ("gt", '>'),
        ("lt", '<'),
        ("nbsp", ' '),
        ("quot", '"'),
    ]
    .into_iter()
    .find_map(|(entity, character)| name.eq_ignore_ascii_case(entity).then_some(character))
}

#[cfg(test)]
mod tests;
