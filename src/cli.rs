use lexopt::prelude::*;
use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;

pub const HELP: &str = "\
Usage: alvin9999-sub [-w DIR | -W] [-v FILE] [-c FILE]

  -w, --write DIR          Write files to DIR (default: ./data)
  -W, --no-write           Write selected formats to stdout, V2Ray first
  -v, --enable-v2ray FILE  Emit V2Ray subscription links
  -c, --enable-clash FILE  Emit a Clash configuration with embedded nodes
  -h, --help               Show this help

With no format selected, emit both sub.txt and sub.yaml.
With --no-write, format filenames are optional and ignored.";

pub struct Options {
    pub directory: Option<PathBuf>,
    pub v2ray: Option<OsString>,
    pub clash: Option<OsString>,
}

pub fn parse(mut parser: lexopt::Parser) -> Result<Option<Options>, Box<dyn Error>> {
    let mut directory = None;
    let mut stdout = false;
    let mut v2ray = None;
    let mut clash = None;
    while let Some(arg) = parser.next()? {
        match arg {
            Short('h') | Long("help") => return Ok(None),
            Short('W') | Long("no-write") => stdout = true,
            Short('w') | Long("write") => directory = Some(PathBuf::from(parser.value()?)),
            Short('v') | Long("enable-v2ray") | Short('c') | Long("enable-clash") => {
                let format = match arg {
                    Short('v') | Long("enable-v2ray") => &mut v2ray,
                    _ => &mut clash,
                };
                let value = parser.optional_value().or_else(|| {
                    parser
                        .try_raw_args()?
                        .next_if(|arg| !arg.as_encoded_bytes().starts_with(b"-"))
                });
                *format = Some(value.unwrap_or_default());
            }
            _ => return Err(arg.unexpected().into()),
        }
    }
    if stdout && directory.is_some() {
        return Err("--write and --no-write cannot be used together".into());
    }
    if v2ray.is_none() && clash.is_none() {
        v2ray = Some("sub.txt".into());
        clash = Some("sub.yaml".into());
    }
    if !stdout {
        for filename in v2ray.iter().chain(&clash) {
            if std::path::Path::new(filename).file_name() != Some(filename.as_os_str()) {
                return Err(
                    "each selected format requires a filename within the output directory".into(),
                );
            }
        }
        if v2ray == clash {
            return Err("the two formats require different filenames".into());
        }
    }
    Ok(Some(Options {
        directory: if stdout {
            None
        } else {
            Some(directory.unwrap_or_else(|| "./data".into()))
        },
        v2ray,
        clash,
    }))
}
