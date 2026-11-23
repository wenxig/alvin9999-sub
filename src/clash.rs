use serde_json::{Value, json};
use std::error::Error;
use subconverter_core::generator::{ExtraSettings, clash::proxy_to_clash};
use subconverter_core::parser::explode;
use subconverter_core::utils::base64::url_safe_base64_decode;

// The upstream Clash writer omits these share-link options from its output.
fn preserve_link_options(link: &str, node: &mut Value) -> Result<(), Box<dyn Error>> {
    if let Some(encoded) = link.strip_prefix("vmess://")
        && let Ok(fields) = serde_json::from_str::<Value>(&url_safe_base64_decode(encoded))
    {
        if let Some(cipher) = fields["scy"].as_str().filter(|s| !s.is_empty()) {
            node["cipher"] = json!(cipher);
        }
        if node["tls"] == true {
            node["client-fingerprint"] = json!(
                fields["fp"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("chrome")
            );
            if let Some(alpn) = fields["alpn"].as_str().filter(|s| !s.is_empty()) {
                node["alpn"] = json!(alpn.split(',').collect::<Vec<_>>());
            }
            node["skip-cert-verify"] = json!(
                matches!(fields["insecure"].as_str(), Some("1" | "true"))
                    || fields["insecure"] == true
                    || fields["insecure"] == 1
            );
        }
    } else if node["type"] == "anytls" {
        let address = link.split('#').next().unwrap_or(link);
        node["client-fingerprint"] = json!("chrome");
        if let Some((_, query)) = address.split_once('?') {
            for (key, value) in form_urlencoded::parse(query.as_bytes()) {
                match key.as_ref() {
                    "fp" if !value.is_empty() => node["client-fingerprint"] = json!(value),
                    "sni" => node["sni"] = json!(value),
                    "alpn" => node["alpn"] = json!(value.split(',').collect::<Vec<_>>()),
                    "allowInsecure" if matches!(value.as_ref(), "1" | "true") => {
                        node["skip-cert-verify"] = json!(true)
                    }
                    _ => {}
                }
            }
        }
        let authority = address
            .trim_start_matches("anytls://")
            .split('?')
            .next()
            .unwrap_or_default();
        if let Some((userinfo, _)) = authority.rsplit_once('@') {
            node["password"] = json!(percent_encoding::percent_decode_str(userinfo).decode_utf8()?);
        }
    }
    Ok(())
}

pub fn render(links: &[String]) -> Result<String, Box<dyn Error>> {
    let mut nodes = Vec::with_capacity(links.len());
    for (index, link) in links.iter().enumerate() {
        nodes.push(
            explode(link)
                .ok_or_else(|| format!("Invalid or unsupported Clash node {}", index + 1))?,
        );
    }
    let settings = ExtraSettings {
        clash_new_field_name: true,
        ..ExtraSettings::default()
    };
    let document = proxy_to_clash(&mut nodes, &[], false, &settings);
    let mut document: Value = serde_saphyr::from_str(&document)?;
    let payload = document["proxies"]
        .as_array_mut()
        .ok_or("missing Clash nodes")?;
    if payload.len() != links.len() {
        return Err("some nodes could not be exported to Clash".into());
    }
    for (link, node) in links.iter().zip(payload.iter_mut()) {
        preserve_link_options(link, node)?;
    }
    // Keep deterministic field ordering and omit empty optional SSR parameters.
    for node in payload.iter_mut() {
        if node["protocol-param"] == "" {
            node.as_object_mut().unwrap().remove("protocol-param");
        }
        node.sort_all_objects();
    }
    Ok(include_str!("clash.yaml")
        .trim_end()
        .replace("${NODES}", &serde_json::to_string(payload)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_share_link_tls_options() {
        let fields = json!({"v":"2", "ps":"fixture", "add":"localhost", "port":443,
            "id":"2fb377bf-c6b6-43e8-b890-06386b598fe4", "net":"ws", "tls":"tls",
            "scy":"chacha20-poly1305","fp":"firefox","insecure":"1","alpn":"h2,http/1.1"});
        let encoded = subconverter_core::utils::base64::base64_encode(&fields.to_string());
        let config = render(&[
            format!("vmess://{encoded}"),
            "anytls://p%40ss@example.org:443?allowInsecure=1&fp=safari&alpn=h2#fixture".into(),
        ])
        .unwrap();
        let parsed: Value = serde_saphyr::from_str(&config).unwrap();
        let nodes = &parsed["proxy-providers"]["nodes"]["payload"];
        assert_eq!(nodes[0]["cipher"], "chacha20-poly1305");
        assert_eq!(nodes[0]["client-fingerprint"], "firefox");
        assert_eq!(nodes[0]["alpn"], json!(["h2", "http/1.1"]));
        assert_eq!(nodes[0]["skip-cert-verify"], true);
        assert_eq!(nodes[1]["client-fingerprint"], "safari");
        assert_eq!(nodes[1]["password"], "p@ss");
        assert_eq!(nodes[1]["skip-cert-verify"], true);
    }

    #[test]
    fn converts_fixture_nodes_into_an_inline_provider() {
        let links: Vec<_> = [
            include_str!("../tests/fixtures/ss.txt"),
            include_str!("../tests/fixtures/v2ray.txt"),
        ]
        .iter()
        .flat_map(|page| page.lines().map(str::to_owned))
        .collect();
        let config = render(&links).unwrap();
        let (_, payload) = config.split_once("type: inline,payload: ").unwrap();
        let nodes: Vec<Value> = serde_json::Deserializer::from_str(payload)
            .into_iter()
            .next()
            .unwrap()
            .unwrap();
        let types: Vec<_> = nodes.iter().map(|n| n["type"].as_str().unwrap()).collect();
        assert_eq!(types, ["ssr", "ss", "vmess", "anytls"]);
        assert_eq!(nodes[2]["network"], "ws");
        assert!(
            nodes
                .iter()
                .all(|n| n["port"].is_number() && n["server"].is_string())
        );
    }
}
