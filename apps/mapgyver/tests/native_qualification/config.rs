use std::collections::BTreeMap;
use std::io::{self, Error};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Workload {
    Q01,
    Q02,
}

impl Workload {
    pub fn extent(self) -> (u32, u32) {
        match self {
            Self::Q01 => (1920, 1080),
            Self::Q02 => (3840, 2160),
        }
    }
    pub fn ceiling(self) -> u64 {
        match self {
            Self::Q01 => 512 * 1024 * 1024,
            Self::Q02 => 1024 * 1024 * 1024,
        }
    }
}

impl FromStr for Workload {
    type Err = io::Error;
    fn from_str(value: &str) -> io::Result<Self> {
        match value.to_ascii_uppercase().as_str() {
            "Q01" => Ok(Self::Q01),
            "Q02" => Ok(Self::Q02),
            _ => Err(Error::other("expected Q01 or Q02")),
        }
    }
}

pub struct Arguments(BTreeMap<String, String>);

impl Arguments {
    pub fn read() -> io::Result<Self> {
        let values = std::env::args().skip(1).collect::<Vec<_>>();
        if values.len() % 2 != 0 {
            return Err(Error::other("arguments require --key value pairs"));
        }
        let mut parsed = BTreeMap::new();
        for pair in values.chunks_exact(2) {
            if !pair[0].starts_with("--") || parsed.insert(pair[0][2..].to_owned(), pair[1].clone()).is_some() {
                return Err(Error::other("invalid or duplicate argument"));
            }
        }
        Ok(Self(parsed))
    }
    pub fn take(&mut self, key: &str, fallback: &str) -> String {
        self.0.remove(key).unwrap_or_else(|| fallback.to_owned())
    }
    pub fn required(&mut self, key: &str) -> io::Result<String> {
        self.0
            .remove(key)
            .ok_or_else(|| Error::other(format!("missing --{key}")))
    }
    pub fn number<T: FromStr>(&mut self, key: &str, fallback: &str) -> io::Result<T> {
        self.take(key, fallback)
            .parse()
            .map_err(|_| Error::other(format!("invalid --{key}")))
    }
    pub fn done(self) -> io::Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(Error::other(format!("unknown arguments: {:?}", self.0.keys())))
        }
    }
}

pub struct RunConfig {
    pub workload: Workload,
    pub adapter_index: usize,
    pub directory: PathBuf,
    pub warmup: Duration,
    pub duration: Duration,
    pub independence: Duration,
    pub trace_capacity: usize,
}

impl RunConfig {
    pub fn read(args: &mut Arguments) -> io::Result<Self> {
        let result = Self {
            workload: args.take("scenario", "Q01").parse()?,
            adapter_index: args.number("adapter", "0")?,
            directory: PathBuf::from(args.required("directory")?),
            warmup: Duration::from_secs(args.number("warmup", "30")?),
            duration: Duration::from_secs(args.number("seconds", "120")?),
            independence: Duration::from_secs(args.number("independence", "0")?),
            trace_capacity: args.number("trace-capacity", "65536")?,
        };
        if result.duration.is_zero()
            || [result.warmup, result.duration, result.independence]
                .iter()
                .any(|duration| duration.as_secs() > 600)
            || result.total().as_secs() > 600
            || !(1..=65536).contains(&result.trace_capacity)
        {
            return Err(Error::other("invalid bounded duration or trace capacity"));
        }
        Ok(result)
    }
    pub fn total(&self) -> Duration {
        self.warmup + self.duration + self.independence * 2
    }
}
