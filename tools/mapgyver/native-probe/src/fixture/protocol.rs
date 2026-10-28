use mapgyver_native_probe::resource_log;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Request {
    Stop,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Response {
    Ready {
        process_id: u32,
        adapter_luid: String,
        channel: String,
    },
    Resources {
        message: resource_log::Message,
    },
    Finished {
        report: Box<Report>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub process_id: u32,
    pub adapter_luid: String,
    pub channel: String,
    pub frames_sent: u32,
    pub resources: Option<resource_log::Report>,
}
