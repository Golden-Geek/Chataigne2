mod discovery;
mod error;
mod stream;

use std::str::FromStr;

use cpal::{DeviceId, HostId, traits::HostTrait};

use crate::{
    AudioBackend, AudioBackendState, AudioDeviceDescriptor, AudioDeviceTargetId, AudioDirection, AudioError,
    AudioErrorCategory, AudioStream, AudioStreamHandler, BackendDescriptor, BackendId, DeviceNegotiator, StreamRequest,
};

use self::{
    discovery::{descriptor_for_device, discover_host},
    error::{backend_state_for, map_cpal_error},
    stream::open_cpal_stream,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CpalBackend {
    host_id: HostId,
}

impl CpalBackend {
    #[must_use]
    const fn new(host_id: HostId) -> Self {
        Self { host_id }
    }

    #[must_use]
    fn backend_id(self) -> BackendId {
        backend_id_for_host(self.host_id)
    }

    fn host(self) -> Result<cpal::Host, AudioError> {
        cpal::host_from_id(self.host_id)
            .map_err(|error| map_cpal_error(error, "initialize audio host", Some(self.backend_id())))
    }
}

#[must_use]
pub fn compiled_cpal_backends() -> Vec<Box<dyn AudioBackend>> {
    cpal::ALL_HOSTS
        .iter()
        .copied()
        .map(|host_id| Box::new(CpalBackend::new(host_id)) as Box<dyn AudioBackend>)
        .collect()
}

#[must_use]
pub fn probe_cpal_backends() -> Vec<BackendDescriptor> {
    compiled_cpal_backends()
        .into_iter()
        .map(|backend| backend.descriptor())
        .collect()
}

impl AudioBackend for CpalBackend {
    fn id(&self) -> BackendId {
        self.backend_id()
    }

    fn descriptor(&self) -> BackendDescriptor {
        let id = self.backend_id();
        let available = cpal::available_hosts().contains(&self.host_id);
        let (state, detail) = if available {
            match cpal::host_from_id(self.host_id) {
                Ok(host) => match host.devices() {
                    Ok(mut devices) => {
                        if devices.next().is_some() {
                            (AudioBackendState::Available, None)
                        } else {
                            empty_host_state_for(self.host_id)
                        }
                    }
                    Err(error) => backend_state_for(self.host_id, &error),
                },
                Err(error) => backend_state_for(self.host_id, &error),
            }
        } else {
            unavailable_state_for(self.host_id)
        };
        BackendDescriptor {
            id,
            label: self.host_id.name().to_owned(),
            state,
            detail,
        }
    }

    fn discover(&self) -> Result<Vec<AudioDeviceDescriptor>, AudioError> {
        discover_host(self.host_id, &self.host()?)
    }

    fn open_stream(&self, request: &StreamRequest) -> Result<Box<dyn AudioStream>, AudioError> {
        self.open_stream_with_handler(request, Box::new(SilenceHandler))
    }

    fn supports_stream_handlers(&self) -> bool {
        true
    }

    fn open_stream_with_handler(
        &self,
        request: &StreamRequest,
        handler: Box<dyn AudioStreamHandler>,
    ) -> Result<Box<dyn AudioStream>, AudioError> {
        request.validate()?;
        if request.target.backend() != &self.backend_id() {
            return Err(AudioError::new(
                AudioErrorCategory::DeviceMissing,
                "audio target belongs to another backend",
            ));
        }
        let host = self.host()?;
        let device = select_device(&host, self.host_id, &request.target, request.direction)?;
        let descriptor = descriptor_for_device(self.host_id, &host, &device)?;
        let format = DeviceNegotiator.negotiate(&descriptor, request.negotiation_request())?;
        open_cpal_stream(device, request, descriptor, format, handler)
    }
}

#[derive(Debug)]
struct SilenceHandler;

impl AudioStreamHandler for SilenceHandler {}

fn select_device(
    host: &cpal::Host,
    host_id: HostId,
    target: &AudioDeviceTargetId,
    direction: AudioDirection,
) -> Result<cpal::Device, AudioError> {
    match target {
        AudioDeviceTargetId::SystemDefault { .. } => match direction {
            AudioDirection::Input => host.default_input_device(),
            AudioDirection::Output => host.default_output_device(),
        }
        .ok_or_else(|| {
            AudioError::new(
                AudioErrorCategory::DeviceMissing,
                "system default audio device is missing",
            )
        }),
        AudioDeviceTargetId::Device { device, .. } => {
            let native_id = DeviceId::from_str(device.as_str()).map_err(|error| {
                map_cpal_error(
                    error,
                    "parse persisted audio device identifier",
                    Some(backend_id_for_host(host_id)),
                )
            })?;
            if native_id.host() != host_id {
                return Err(AudioError::new(
                    AudioErrorCategory::DeviceMissing,
                    "persisted audio device belongs to a different host",
                ));
            }
            host.device_by_id(&native_id).ok_or_else(|| {
                AudioError::new(
                    AudioErrorCategory::DeviceMissing,
                    "selected audio device is not currently connected",
                )
            })
        }
    }
}

fn backend_id_for_host(host_id: HostId) -> BackendId {
    let id = if host_id.to_string() == "null" {
        "cpal-null".to_owned()
    } else {
        host_id.to_string()
    };
    BackendId::new(id).expect("CPAL host identifiers are non-empty and trimmed")
}

fn empty_host_state_for(host_id: HostId) -> (AudioBackendState, Option<String>) {
    match host_id.to_string().as_str() {
        "asio" => (
            AudioBackendState::MissingDriver,
            Some("The ASIO host loaded, but it reported no installed drivers.".to_owned()),
        ),
        "jack" | "pipewire" => (
            AudioBackendState::MissingServer,
            Some("The audio server host loaded, but it reported no devices.".to_owned()),
        ),
        _ => (AudioBackendState::Available, None),
    }
}

fn unavailable_state_for(host_id: HostId) -> (AudioBackendState, Option<String>) {
    match host_id.to_string().as_str() {
        "asio" => (
            AudioBackendState::MissingDriver,
            Some("ASIO support is compiled, but no ASIO driver is available.".to_owned()),
        ),
        "jack" => (
            AudioBackendState::MissingServer,
            Some("JACK support is compiled, but the JACK library or server is unavailable.".to_owned()),
        ),
        "pipewire" => (
            AudioBackendState::MissingServer,
            Some("Native PipeWire support is compiled, but the PipeWire server is unavailable.".to_owned()),
        ),
        _ => (
            AudioBackendState::Unavailable,
            Some("The compiled audio host is unavailable on this system.".to_owned()),
        ),
    }
}

#[cfg(test)]
mod tests;
