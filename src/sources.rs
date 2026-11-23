use std::borrow::Cow;
use std::env;
use std::error::Error;
use std::io;
use std::time::Duration;

use curl::easy::{Easy2, Handler, WriteError};
use curl::multi::Multi;

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

#[derive(Default)]
struct Page(Vec<u8>);

impl Handler for Page {
    fn write(&mut self, bytes: &[u8]) -> Result<usize, WriteError> {
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
}

pub fn fetch() -> Result<[Vec<u8>; SOURCES.len()], Box<dyn Error>> {
    let client = Multi::new();
    let mut transfers = Vec::with_capacity(SOURCES.len());
    for (variable, default_url) in SOURCES {
        let url = env::var(variable).map_or(Cow::Borrowed(default_url), Cow::Owned);
        let mut request = Easy2::new(Page::default());
        request.url(&url)?;
        request.follow_location(true)?;
        request.max_redirections(20)?;
        request.fail_on_error(true)?;
        request.accept_encoding("")?;
        request.timeout(Duration::from_secs(50))?;
        transfers.push(client.add2(request)?);
    }

    loop {
        let active = client.perform()?;
        let mut result = Ok(());
        client.messages(|message| {
            for (index, transfer) in transfers.iter().enumerate() {
                if let Some(Err(error)) = message.result_for2(transfer) {
                    result = Err(io::Error::other(format!("{}: {error}", SOURCES[index].0)));
                }
            }
        });
        result?;
        if active == 0 {
            break;
        }
        client.poll(&mut [], Duration::from_secs(1))?;
    }

    let mut pages = [const { Vec::new() }; SOURCES.len()];
    for (index, transfer) in transfers.into_iter().enumerate() {
        let mut response = client.remove2(transfer)?;
        let status = response.response_code()?;
        if !(200..300).contains(&status) {
            return Err(io::Error::other(format!("{}: HTTP {status}", SOURCES[index].0)).into());
        }
        pages[index] = std::mem::take(&mut response.get_mut().0);
    }
    Ok(pages)
}
