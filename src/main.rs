mod extract;
mod sources;

use std::env;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let pages = sources::fetch()?;
    let repository = env::var("GITHUB_REPOSITORY").unwrap_or_else(|_| "wenxig/alvin9999-sub".into());
    let config = include_str!("clash.yaml")
        .trim_end_matches('\n')
        .replace("${REPOSITORY}", &repository);

    let out_dir = Path::new("data");
    fs::create_dir_all(out_dir)?;
    let mut output = Vec::new();
    let mut separator = "";
    for page in pages {
        let html = String::from_utf8_lossy(&page);
        extract::subscriptions(&html, |subscription| {
            output.write_all(separator.as_bytes())?;
            output.write_all(subscription.as_bytes())?;
            separator = "\n";
            Ok(())
        })?;
    }
    fs::write(out_dir.join("sub.txt"), output)?;
    fs::write(out_dir.join("sub.yaml"), config)?;
    Ok(())
}
