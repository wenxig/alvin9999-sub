use std::env;
use std::io::{self, Read};
use std::thread;
use std::time::Duration;

const SOURCES: [(&str, &str); 2] = [
    (
        "SS_SOURCE_URL",
        "https://alvin9999.com/ss%E5%85%8D%E8%B4%B9%E8%B4%A6%E5%8F%B7/",
    ),
    (
        "V2RAY_SOURCE_URL",
        "https://alvin9999.com/v2ray%E5%85%8D%E8%B4%B9%E8%B4%A6%E5%8F%B7/",
    ),
];

fn fetch_page(agent: &ureq::Agent, (variable, default_url): (&str, &str)) -> io::Result<Vec<u8>> {
    let url = env::var(variable).unwrap_or_else(|_| default_url.to_owned());
    let mut response = agent.get(&url).call().map_err(io::Error::other)?;
    if !response.status().is_success() {
        return Err(io::Error::other(format!("HTTP {}", response.status())));
    }
    let mut bytes = Vec::new();
    response.body_mut().as_reader().read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub fn fetch() -> io::Result<[Vec<u8>; SOURCES.len()]> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(20)
        .timeout_global(Some(Duration::from_secs(50)))
        .build()
        .into();
    thread::scope(|scope| {
        let requests = SOURCES.map(|source| {
            let agent = &agent;
            scope.spawn(move || {
                fetch_page(agent, source)
                    .map_err(|e| io::Error::other(format!("{}: {e}", source.0)))
            })
        });
        let [ss, v2ray] = requests.map(|request| {
            request
                .join()
                .map_err(|_| io::Error::other("source worker panicked"))?
        });
        Ok([ss?, v2ray?])
    })
}
