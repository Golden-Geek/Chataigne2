use serde::{Deserialize, Serialize};

pub(crate) const DMX_SLOT_COUNT: usize = 512;
pub(crate) const SACN_MAX_UNIVERSE: u16 = 63_999;
pub(crate) const ARTNET_MAX_UNIVERSE: u16 = 32_768;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub(crate) struct DmxFrame {
    pub universe: u16,
    #[serde(default)]
    pub sequence: u8,
    #[serde(default = "default_priority")]
    pub priority: u8,
    pub slots: Vec<u8>,
}

impl DmxFrame {
    pub(crate) fn new(universe: u16, slots: Vec<u8>) -> Result<Self, String> {
        Self::with_metadata(universe, 0, default_priority(), slots)
    }

    pub(crate) fn with_metadata(
        universe: u16,
        sequence: u8,
        priority: u8,
        slots: Vec<u8>,
    ) -> Result<Self, String> {
        if universe == 0 || universe > SACN_MAX_UNIVERSE {
            return Err(format!(
                "DMX universe must be between 1 and {SACN_MAX_UNIVERSE}"
            ));
        }
        if priority == 0 || priority > 200 {
            return Err("sACN priority must be between 1 and 200".to_string());
        }
        if slots.len() > DMX_SLOT_COUNT {
            return Err(format!(
                "DMX frame has {} slots; at most {DMX_SLOT_COUNT} are allowed",
                slots.len()
            ));
        }

        Ok(Self {
            universe,
            sequence,
            priority,
            slots,
        })
    }

    pub(crate) fn set_channel(&mut self, channel: u16, value: u8) -> Result<(), String> {
        if !(1..=DMX_SLOT_COUNT as u16).contains(&channel) {
            return Err(format!(
                "DMX channel must be between 1 and {DMX_SLOT_COUNT}"
            ));
        }
        self.slots.resize(DMX_SLOT_COUNT, 0);
        self.slots[usize::from(channel - 1)] = value;
        Ok(())
    }

    pub(crate) fn blackout(universe: u16) -> Result<Self, String> {
        Self::new(universe, vec![0; DMX_SLOT_COUNT])
    }
}

pub(crate) fn parse_slots_json(value: &str) -> Result<Vec<u8>, String> {
    let slots = serde_json::from_str::<Vec<u16>>(value)
        .map_err(|error| format!("DMX frame must be a JSON array of channel values: {error}"))?;
    if slots.len() > DMX_SLOT_COUNT {
        return Err(format!(
            "DMX frame has {} slots; at most {DMX_SLOT_COUNT} are allowed",
            slots.len()
        ));
    }
    slots
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            u8::try_from(value).map_err(|_| {
                format!(
                    "DMX channel {} value {value} is outside the 0..255 range",
                    index + 1
                )
            })
        })
        .collect()
}

pub(crate) fn slots_json(slots: &[u8]) -> String {
    serde_json::to_string(slots).expect("DMX byte arrays are always JSON serializable")
}

const fn default_priority() -> u8 {
    100
}

#[cfg(test)]
mod tests;
