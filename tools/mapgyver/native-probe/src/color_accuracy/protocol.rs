//! Bounded private child control/reporting. No frame pixels or native handles cross pipes.

use super::DeviceReport;
use mapgyver_native_probe::color_reference::{Comparison, Fixture};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    SourceA,
    SourceB,
    Consumer,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Self::SourceA => "source_a",
            Self::SourceB => "source_b",
            Self::Consumer => "consumer",
        }
    }
    pub fn channel(self) -> Option<u32> {
        match self {
            Self::SourceA => Some(0),
            Self::SourceB => Some(1),
            Self::Consumer => None,
        }
    }
}

impl std::str::FromStr for Role {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "source_a" => Ok(Self::SourceA),
            "source_b" => Ok(Self::SourceB),
            "consumer" => Ok(Self::Consumer),
            _ => Err("invalid color child role".into()),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Request {
    Stop,
    Capture,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Response {
    Ready {
        process_id: u32,
        adapter_luid: String,
        role: Role,
        fixture: Fixture,
    },
    Finished {
        report: Box<Report>,
    },
}

#[derive(Serialize, Deserialize)]
pub struct Report {
    pub process_id: u32,
    pub adapter_luid: String,
    pub role: Role,
    pub fixture: Fixture,
    pub initial_pixel_upload_bytes: u64,
    pub full_image_readbacks: u32,
    pub sender_unregistered: Option<bool>,
    pub device: DeviceReport,
    pub comparison: Option<Comparison>,
}

impl Report {
    pub fn validate_identity(
        &self,
        role: Role,
        pid: u32,
        luid: &str,
        scenario: mapgyver_native_probe::workload::Scenario,
        fixture: Fixture,
    ) -> Result<(), String> {
        if self.process_id != pid
            || pid == 0
            || self.adapter_luid != luid
            || self.role != role
            || self.device.role != role.name()
            || self.fixture != fixture
        {
            return Err("stale or invalid accuracy child identity".into());
        }
        if let Some(channel) = role.channel() {
            let [width, height] = if channel == 0 { scenario.extent() } else { [1280, 720] };
            let bytes = if fixture == Fixture::Authored {
                u64::from(width) * u64::from(height) * 4
            } else {
                0
            };
            if self.initial_pixel_upload_bytes != bytes
                || self.full_image_readbacks != 0
                || self.comparison.is_some()
                || self.sender_unregistered.is_none()
                || self.device.operations.uniform_upload_bytes != if fixture == Fixture::Procedural { 32 } else { 0 }
                || self.device.operations.shader_draws != u64::from(fixture == Fixture::Procedural)
            {
                return Err("invalid producer accuracy metadata".into());
            }
        } else {
            let c = self.comparison.as_ref().ok_or("consumer comparison missing")?;
            let [width, height] = scenario.extent();
            let count = u64::from(width) * u64::from(height);
            if self.initial_pixel_upload_bytes != 0
                || self.full_image_readbacks != 1
                || self.sender_unregistered.is_some()
                || c.extent != [width, height]
                || c.compared_pixels != count
                || c.tolerance_per_component != mapgyver_native_probe::color_reference::TOLERANCE
                || c.mismatched_pixels > count
                || c.first_differences.len() as u64 != c.mismatched_pixels.min(16)
            {
                return Err("invalid consumer accuracy coverage".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
