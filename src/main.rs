mod clash;
mod cli;
mod extract;
mod sources;

use std::error::Error;
use std::fs;
use std::io::{self, Write};

fn main() -> Result<(), Box<dyn Error>> {
    let Some(options) = cli::parse(lexopt::Parser::from_env())? else {
        println!("{}", cli::HELP);
        return Ok(());
    };
    let mut nodes = Vec::new();
    for page in sources::fetch()? {
        extract::subscriptions(&String::from_utf8_lossy(&page), |link| {
            nodes.push(link.to_owned());
            Ok(())
        })?;
    }
    let mut outputs = Vec::new();
    if let Some(filename) = options.v2ray {
        outputs.push((filename, nodes.join("\n")));
    }
    if let Some(filename) = options.clash {
        outputs.push((filename, clash::render(&nodes)?));
    }
    if let Some(directory) = options.directory {
        fs::create_dir_all(&directory)?;
        for (filename, contents) in outputs {
            fs::write(directory.join(filename), contents)?;
        }
    } else {
        let mut stdout = io::stdout().lock();
        for (_, contents) in outputs {
            writeln!(stdout, "{contents}")?;
        }
    }
    Ok(())
}
