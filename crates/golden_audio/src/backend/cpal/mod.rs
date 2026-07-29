#[cfg(all(windows, feature = "asio"))]
mod asio;
mod discovery;
mod error;
mod stream;

use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex},
};

use cpal::{DeviceId, HostId, traits::HostTrait};

use crate::{
    AudioBackend, AudioBackendState, AudioDeviceDescriptor, AudioDeviceInventory, AudioDeviceTargetId, AudioDirection,
    AudioError, AudioErrorCategory, AudioStream, AudioStreamHandler, BackendDescriptor, BackendId, DeviceNegotiator,
    StreamRequest,
};

use self::{
    discovery::{descriptor_for_device, descriptor_for_exact_device, discover_host_inventory},
    error::{backend_state_for, map_cpal_error},
    stream::open_cpal_stream,
};

#[derive(Clone, Debug)]
struct CpalBackend {
    host_id: HostId,
    device_cache: Arc<Mutex<HashMap<AudioDeviceTargetId, cpal::Device>>>,
}

/// Side-effect-free metadata for a native audio host compiled into this build.
///
/// Reading the catalog never initializes a host or enumerates devices. Callers can
/// therefore present a driver choice without waking every installed audio stack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeAudioBackendInfo {
    pub id: BackendId,
    pub label: String,
    pub is_platform_default: bool,
    pub supports_system_default: bool,
}

impl CpalBackend {
    #[must_use]
    fn new(host_id: HostId) -> Self {
        Self {
            host_id,
            device_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[must_use]
    fn backend_id(&self) -> BackendId {
        backend_id_for_host(self.host_id)
    }

    fn host(&self) -> Result<cpal::Host, AudioError> {
        cpal::host_from_id(self.host_id)
            .map_err(|error| map_cpal_error(error, "initialize audio host", Some(self.backend_id())))
    }

    fn cached_device(&self, target: &AudioDeviceTargetId) -> Result<Option<cpal::Device>, AudioError> {
        self.device_cache
            .lock()
            .map(|cache| cache.get(target).cloned())
            .map_err(|_| {
                AudioError::new(
                    AudioErrorCategory::InternalInvariant,
                    "CPAL device cache lock is poisoned",
                )
            })
    }

    fn cache_device(&self, target: AudioDeviceTargetId, device: cpal::Device) -> Result<(), AudioError> {
        self.device_cache
            .lock()
            .map(|mut cache| {
                if is_asio_host(self.host_id) {
                    cache.retain(|candidate, _| candidate == &target);
                }
                cache.insert(target, device);
            })
            .map_err(|_| {
                AudioError::new(
                    AudioErrorCategory::InternalInvariant,
                    "CPAL device cache lock is poisoned",
                )
            })
    }

    #[cfg(all(windows, feature = "asio"))]
    fn exact_device(
        &self,
        host: &cpal::Host,
        target: &AudioDeviceTargetId,
    ) -> Result<Option<cpal::Device>, AudioError> {
        let AudioDeviceTargetId::Device { .. } = target else {
            return Ok(None);
        };
        let native_id = native_device_id(self.host_id, target)?;
        Ok(host.device_by_id(&native_id))
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
pub fn compiled_cpal_backend_catalog() -> Vec<NativeAudioBackendInfo> {
    cpal::ALL_HOSTS
        .iter()
        .copied()
        .filter(|host_id| host_id.to_string() != "null")
        .map(|host_id| {
            let name = host_id.to_string();
            NativeAudioBackendInfo {
                id: backend_id_for_host(host_id),
                label: host_id.name().to_owned(),
                is_platform_default: is_platform_default_host(&name),
                supports_system_default: name != "asio",
            }
        })
        .collect()
}

/// Constructs one exact compiled native backend without initializing it.
#[must_use]
pub fn cpal_backend_by_id(id: &BackendId) -> Option<Box<dyn AudioBackend>> {
    cpal::ALL_HOSTS.iter().copied().find_map(|host_id| {
        (backend_id_for_host(host_id) == *id).then(|| Box::new(CpalBackend::new(host_id)) as Box<dyn AudioBackend>)
    })
}

#[must_use]
pub fn probe_cpal_backends() -> Vec<BackendDescriptor> {
    compiled_cpal_backends()
        .into_iter()
        .map(|backend| backend.descriptor())
        .collect()
}

fn is_platform_default_host(name: &str) -> bool {
    name == platform_default_host_name()
}

const fn platform_default_host_name() -> &'static str {
    #[cfg(windows)]
    {
        "wasapi"
    }
    #[cfg(target_vendor = "apple")]
    {
        "coreaudio"
    }
    #[cfg(target_os = "android")]
    {
        "aaudio"
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd"
    ))]
    {
        "alsa"
    }
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    {
        "webaudio"
    }
    #[cfg(not(any(
        windows,
        target_vendor = "apple",
        target_os = "android",
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        all(target_arch = "wasm32", target_os = "unknown")
    )))]
    {
        ""
    }
}

impl AudioBackend for CpalBackend {
    fn id(&self) -> BackendId {
        self.backend_id()
    }

    fn descriptor(&self) -> BackendDescriptor {
        let id = self.backend_id();
        #[cfg(all(windows, feature = "asio"))]
        if is_asio_host(self.host_id) {
            let (state, detail) = if asio::has_installed_driver() {
                (AudioBackendState::Available, None)
            } else {
                empty_host_state_for(self.host_id)
            };
            return BackendDescriptor {
                id,
                label: self.host_id.name().to_owned(),
                state,
                detail,
            };
        }
        let (state, detail) = match cpal::host_from_id(self.host_id) {
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
        };
        BackendDescriptor {
            id,
            label: self.host_id.name().to_owned(),
            state,
            detail,
        }
    }

    fn device_inventory(&self) -> Result<AudioDeviceInventory, AudioError> {
        #[cfg(all(windows, feature = "asio"))]
        if is_asio_host(self.host_id) {
            return Ok(asio::lightweight_inventory(&self.backend_id()));
        }
        discover_host_inventory(self.host_id, &self.host()?)
    }

    fn probe_device(&self, target: &AudioDeviceTargetId) -> Result<Option<AudioDeviceDescriptor>, AudioError> {
        if target.backend() != &self.backend_id() {
            return Ok(None);
        }
        #[cfg(all(windows, feature = "asio"))]
        if is_asio_host(self.host_id) {
            let host = self.host()?;
            let Some(device) = self.exact_device(&host, target)? else {
                return Ok(None);
            };
            let descriptor = descriptor_for_exact_device(self.host_id, &device)?;
            self.cache_device(target.clone(), device)?;
            return Ok(Some(descriptor));
        }
        let AudioDeviceTargetId::Device { .. } = target else {
            return Ok(None);
        };
        let host = self.host()?;
        let native_id = native_device_id(self.host_id, target)?;
        let Some(device) = host.device_by_id(&native_id) else {
            return Ok(None);
        };
        descriptor_for_device(self.host_id, &host, &device).map(Some)
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
        let device = if is_asio_host(self.host_id) {
            match self.cached_device(&request.target)? {
                Some(device) => device,
                None => {
                    let device = select_device(&host, self.host_id, &request.target, request.direction)?;
                    self.cache_device(request.target.clone(), device.clone())?;
                    device
                }
            }
        } else {
            select_device(&host, self.host_id, &request.target, request.direction)?
        };
        let descriptor = if is_asio_host(self.host_id) {
            descriptor_for_exact_device(self.host_id, &device)?
        } else {
            descriptor_for_device(self.host_id, &host, &device)?
        };
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
        AudioDeviceTargetId::Device { .. } => {
            let native_id = native_device_id(host_id, target)?;
            host.device_by_id(&native_id).ok_or_else(|| {
                AudioError::new(
                    AudioErrorCategory::DeviceMissing,
                    "selected audio device is not currently connected",
                )
            })
        }
    }
}

fn native_device_id(host_id: HostId, target: &AudioDeviceTargetId) -> Result<DeviceId, AudioError> {
    let AudioDeviceTargetId::Device { device, .. } = target else {
        return Err(AudioError::new(
            AudioErrorCategory::DeviceMissing,
            "system-default targets do not identify one exact device",
        ));
    };
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
    Ok(native_id)
}

fn is_asio_host(host_id: HostId) -> bool {
    host_id.to_string() == "asio"
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

#[cfg(test)]
mod tests;
