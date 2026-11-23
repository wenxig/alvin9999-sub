use super::subscriptions;
use std::io;

const SS: &str = include_str!("../../tests/fixtures/ss.html");
const V2RAY: &str = include_str!("../../tests/fixtures/v2ray.html");
const SS_NODES: &str = include_str!("../../tests/fixtures/ss.txt");
const V2RAY_NODES: &str = include_str!("../../tests/fixtures/v2ray.txt");

fn nodes(html: &str) -> io::Result<String> {
    let mut result = Vec::new();
    subscriptions(html, |node| {
        result.push(node.to_owned());
        Ok(())
    })?;
    Ok(result.join("\n"))
}

#[test]
fn complete_pages_match_expected_links_in_order() {
    assert_eq!(nodes(SS).unwrap(), SS_NODES);
    assert_eq!(nodes(V2RAY).unwrap(), V2RAY_NODES);
}

#[test]
fn extra_classes_and_single_quoted_attributes() {
    let html = V2RAY.replace(
        "class=\"wp-block-code\"",
        "id='import' class = 'highlight wp-block-code'",
    );
    assert_eq!(nodes(&html).unwrap(), V2RAY_NODES);
}

#[test]
fn crlf_plain_code_and_alternate_entities() {
    let html = SS
        .replace("<code>", "<code>\r\n ")
        .replace("</code>", " \r\n</code>")
        .replace("&#91;", "&#x5b;")
        .replace("&#93;", "]");
    assert_eq!(nodes(&html).unwrap(), SS_NODES);
}

#[test]
fn ignores_links_in_other_blocks_and_outside_article() {
    let html = SS.replace("</article>", "<pre class='other'>ss://unrelated</pre></article><pre class='wp-block-code'>ss://sidebar</pre>");
    assert_eq!(nodes(&html).unwrap(), SS_NODES);
}

#[test]
fn fails_when_the_page_loses_its_code_blocks() {
    assert_eq!(
        nodes(&SS.replace("wp-block-code", "wp-block-table"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
}
