use std::time::Duration;

const DEFAULT_FRONTEND_PORT: u16 = 5173;
const DEFAULT_BACKEND_PORT: u16 = 7010;
const DEFAULT_FRONTEND_TIMEOUT_SECS: u64 = 60;
const DEFAULT_BACKEND_TIMEOUT_SECS: u64 = 300;
const DEFAULT_ENGINE_TIMEOUT_SECS: u64 = 30;
const DEFAULT_POLL_MILLIS: u64 = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchConfig {
    pub frontend_port: u16,
    pub backend_port: u16,
    pub frontend_timeout: Duration,
    pub backend_timeout: Duration,
    pub engine_timeout: Duration,
    pub poll_interval: Duration,
    pub headless: bool,
    pub app_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseOutcome {
    Help,
    Watch(WatchConfig),
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            frontend_port: DEFAULT_FRONTEND_PORT,
            backend_port: DEFAULT_BACKEND_PORT,
            frontend_timeout: Duration::from_secs(DEFAULT_FRONTEND_TIMEOUT_SECS),
            backend_timeout: Duration::from_secs(DEFAULT_BACKEND_TIMEOUT_SECS),
            engine_timeout: Duration::from_secs(DEFAULT_ENGINE_TIMEOUT_SECS),
            poll_interval: Duration::from_millis(DEFAULT_POLL_MILLIS),
            headless: false,
            app_args: Vec::new(),
        }
    }
}

impl WatchConfig {
    pub fn parse<I, S>(args: I) -> Result<ParseOutcome, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into).peekable();
        let Some(command) = args.next() else {
            return Err(format!("missing xtask command\n\n{}", usage()));
        };

        if matches!(command.as_str(), "-h" | "--help" | "help") {
            return Ok(ParseOutcome::Help);
        }
        if command != "watch" {
            return Err(format!("unknown xtask command '{command}'\n\n{}", usage()));
        }

        let mut config = Self::default();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "-h" | "--help" => return Ok(ParseOutcome::Help),
                "--frontend-port" => {
                    config.frontend_port = parse_port(&argument, take_value(&mut args, &argument)?)?;
                }
                "--backend-port" => {
                    config.backend_port = parse_port(&argument, take_value(&mut args, &argument)?)?;
                }
                "--frontend-timeout-secs" => {
                    config.frontend_timeout =
                        Duration::from_secs(parse_positive(&argument, take_value(&mut args, &argument)?)?);
                }
                "--backend-timeout-secs" => {
                    config.backend_timeout =
                        Duration::from_secs(parse_positive(&argument, take_value(&mut args, &argument)?)?);
                }
                "--engine-timeout-secs" => {
                    config.engine_timeout =
                        Duration::from_secs(parse_positive(&argument, take_value(&mut args, &argument)?)?);
                }
                "--poll-ms" => {
                    config.poll_interval =
                        Duration::from_millis(parse_positive(&argument, take_value(&mut args, &argument)?)?);
                }
                "--headless" => config.headless = true,
                "--" => {
                    config.app_args.extend(args);
                    break;
                }
                _ => {
                    return Err(format!(
                        "unknown watch option '{argument}'; use 'cargo xtask watch --help'"
                    ));
                }
            }
        }

        if config.frontend_port == config.backend_port {
            return Err("frontend and backend ports must be distinct".to_string());
        }

        Ok(ParseOutcome::Watch(config))
    }
}

fn take_value<I>(args: &mut std::iter::Peekable<I>, option: &str) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    args.next().ok_or_else(|| format!("{option} requires a value"))
}

fn parse_port(option: &str, value: String) -> Result<u16, String> {
    let port = value
        .parse::<u16>()
        .map_err(|_| format!("{option} must be an integer from 1 through 65535, got '{value}'"))?;
    if port == 0 {
        return Err(format!("{option} must be greater than zero"));
    }
    Ok(port)
}

fn parse_positive(option: &str, value: String) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{option} must be a positive integer, got '{value}'"))?;
    if parsed == 0 {
        return Err(format!("{option} must be greater than zero"));
    }
    Ok(parsed)
}

pub fn usage() -> &'static str {
    "Chataigne workspace tasks\n\n\
Usage:\n\
  cargo xtask watch [OPTIONS] [-- APP_ARGS...]\n\n\
Watch options:\n\
  --frontend-port PORT           Vite port (default: 5173)\n\
  --backend-port PORT            golden_core UI host port (default: 7010)\n\
  --frontend-timeout-secs SECS   Frontend startup deadline (default: 60)\n\
  --backend-timeout-secs SECS    Backend startup deadline, including compilation (default: 300)\n\
  --engine-timeout-secs SECS     Engine snapshot deadline (default: 30)\n\
  --poll-ms MILLIS               Readiness polling interval (default: 200)\n\
  --headless                     Pass --headless to the application\n\
  -h, --help                     Print this help\n\n\
The supervisor binds both services to 127.0.0.1, emits JSON events on stdout,\n\
and writes labeled supervisor and child-process logs to stderr.\n"
}
