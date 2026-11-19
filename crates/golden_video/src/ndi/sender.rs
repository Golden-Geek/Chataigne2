//! Worker-owned NDI publication. Synchronous SDK calls keep frame borrows exact.
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    ptr::{NonNull, null},
    sync::{Mutex, OnceLock},
};

use golden_time::RationalRate;
use ndi_sdk::sys;

use crate::{AlphaMode, ColorEncoding, PixelFormat, TextureDescriptor, valid_ndi_sender_name};

use super::{NdiError, Runtime};

fn senders() -> &'static Mutex<BTreeMap<String, Option<String>>> {
    static SENDERS: OnceLock<Mutex<BTreeMap<String, Option<String>>>> = OnceLock::new();
    SENDERS.get_or_init(Mutex::default)
}

/// Exact advertised names owned by this process, excluding senders still opening.
pub fn active_sender_names() -> Vec<String> {
    senders().lock().unwrap().values().flatten().cloned().collect()
}

struct SenderName(String);

impl SenderName {
    fn reserve(name: &str) -> Result<Self, NdiError> {
        let mut senders = senders().lock().unwrap();
        if senders.contains_key(name) {
            return Err(NdiError::Endpoint("an NDI sender with a name already in use"));
        }
        senders.insert(name.to_owned(), None);
        Ok(Self(name.to_owned()))
    }
}

impl Drop for SenderName {
    fn drop(&mut self) {
        senders().lock().unwrap().remove(&self.0);
    }
}

/// Checked, tightly packed progressive SDR NDI video layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NdiVideoFormat {
    descriptor: TextureDescriptor,
    rate: RationalRate,
    bytes: usize,
}

impl NdiVideoFormat {
    /// Admits sRGB BGRA8 pixels with opaque or straight alpha.
    pub fn new(descriptor: TextureDescriptor, rate: RationalRate) -> Result<Self, NdiError> {
        if descriptor.format != PixelFormat::Bgra8Unorm
            || descriptor.color_encoding != ColorEncoding::Srgb
            || !matches!(descriptor.alpha_mode, AlphaMode::Opaque | AlphaMode::Straight)
            || descriptor.extent.width() > i32::MAX as u32 / 4
            || descriptor.extent.height() > i32::MAX as u32
            || rate.numerator() > i32::MAX as u32
            || rate.denominator() > i32::MAX as u32
        {
            return Err(NdiError::Frame("unsupported NDI send layout"));
        }
        let bytes = usize::try_from(
            descriptor
                .byte_len()
                .map_err(|_| NdiError::Frame("send size overflow"))?,
        )
        .map_err(|_| NdiError::Frame("send size overflow"))?;
        Ok(Self {
            descriptor,
            rate,
            bytes,
        })
    }

    /// Total byte count of the tightly packed frame.
    pub fn bytes(self) -> usize {
        self.bytes
    }

    /// Explicit texture contract for the conversion and staging paths.
    pub fn descriptor(self) -> TextureDescriptor {
        self.descriptor
    }

    /// Frame rate advertised to NDI receivers.
    pub fn rate(self) -> RationalRate {
        self.rate
    }
}

/// One SDK sender, created, used and destroyed on a single publication worker.
///
/// The caller owns scheduling. Neither stream is paced by the SDK.
pub struct NdiSender {
    runtime: &'static Runtime,
    instance: NonNull<sys::NDIlib_send_instance_type>,
    name: String,
    _reservation: SenderName,
}

impl NdiSender {
    /// Advertise the local name and retain the exact network name reported by the SDK.
    pub fn new(local_name: &str) -> Result<Self, NdiError> {
        if !valid_ndi_sender_name(local_name) {
            return Err(NdiError::SenderName);
        }
        let reservation = SenderName::reserve(local_name)?;
        let runtime = Runtime::get()?;
        let api = &runtime.api;
        let create = api.send_create.ok_or_else(incomplete_send_api)?;
        let get_name = api.send_get_source_name.ok_or_else(incomplete_send_api)?;
        if api.send_destroy.is_none() || api.send_send_video_v2.is_none() || api.send_send_audio_v3.is_none() {
            return Err(incomplete_send_api());
        }
        let name = CString::new(local_name).map_err(|_| NdiError::SenderName)?;
        let settings = sys::NDIlib_send_create_t {
            p_ndi_name: name.as_ptr(),
            p_groups: null(),
            clock_video: false,
            clock_audio: false,
        };
        // SAFETY: the SDK synchronously copies the NUL-terminated name and settings.
        let instance = NonNull::new(unsafe { create(&settings) }).ok_or(NdiError::Endpoint("an NDI sender"))?;
        let mut sender = Self {
            runtime,
            instance,
            name: String::new(),
            _reservation: reservation,
        };
        // SAFETY: the source and its C string remain valid until this owned sender is destroyed.
        let source = unsafe { get_name(instance.as_ptr()).as_ref() }
            .filter(|source| !source.p_ndi_name.is_null())
            .ok_or(NdiError::Endpoint("an advertised NDI name"))?;
        sender.name = unsafe { CStr::from_ptr(source.p_ndi_name) }
            .to_str()
            .map_err(|_| NdiError::Endpoint("a UTF-8 NDI name"))?
            .to_owned();
        if sender.name.is_empty() {
            return Err(NdiError::Endpoint("a nonempty NDI name"));
        }
        senders()
            .lock()
            .unwrap()
            .insert(local_name.to_owned(), Some(sender.name.clone()));
        Ok(sender)
    }

    /// Exact discoverable SDK name, including its host prefix when supplied.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Submit the frame synchronously on the publication worker. The SDK retains no pixel borrow.
    /// None lets the SDK synthesize the sender's timecode.
    pub fn send_video(
        &mut self,
        format: NdiVideoFormat,
        pixels: &mut [u8],
        timecode_100ns: Option<i64>,
    ) -> Result<(), NdiError> {
        if pixels.len() != format.bytes {
            return Err(NdiError::Frame("send buffer does not match its format"));
        }
        let extent = format.descriptor.extent;
        let frame = sys::NDIlib_video_frame_v2_t {
            xres: extent.width() as i32,
            yres: extent.height() as i32,
            FourCC: match format.descriptor.alpha_mode {
                AlphaMode::Opaque => sys::NDIlib_FourCC_video_type_e::NDIlib_FourCC_video_type_BGRX,
                _ => sys::NDIlib_FourCC_video_type_e::NDIlib_FourCC_video_type_BGRA,
            },
            frame_rate_N: format.rate.numerator() as i32,
            frame_rate_D: format.rate.denominator() as i32,
            picture_aspect_ratio: extent.width() as f32 / extent.height() as f32,
            frame_format_type: sys::NDIlib_frame_format_type_e::NDIlib_frame_format_type_progressive,
            timecode: timecode_100ns.unwrap_or(i64::MAX),
            p_data: pixels.as_mut_ptr(),
            __bindgen_anon_1: sys::NDIlib_video_frame_v2_t__bindgen_ty_1 {
                line_stride_in_bytes: extent.width() as i32 * 4,
            },
            p_metadata: null(),
            timestamp: 0,
        };
        // SAFETY: the synchronous send borrows checked rows only until return.
        unsafe { self.runtime.api.send_send_video_v2.unwrap()(self.instance.as_ptr(), &frame) };
        Ok(())
    }

    /// Submit tightly packed planar samples in NDI's native reference level, on the same worker.
    /// None lets the SDK synthesize the sender's timecode.
    pub fn send_audio(
        &mut self,
        sample_rate: u32,
        channels: usize,
        samples: &mut [f32],
        timecode_100ns: Option<i64>,
    ) -> Result<(), NdiError> {
        let Some(frames) = samples.len().checked_div(channels) else {
            return Err(NdiError::AudioFrame("send requires a nonempty channel layout"));
        };
        if sample_rate == 0
            || sample_rate > i32::MAX as u32
            || channels > super::audio::MAX_AUDIO_CHANNELS
            || frames == 0
            || frames > i32::MAX as usize / size_of::<f32>()
            || samples.len() != frames * channels
            || !samples.iter().all(|sample| sample.is_finite())
        {
            return Err(NdiError::AudioFrame("invalid planar send block"));
        }
        let frame = sys::NDIlib_audio_frame_v3_t {
            sample_rate: sample_rate as i32,
            no_channels: channels as i32,
            no_samples: frames as i32,
            timecode: timecode_100ns.unwrap_or(i64::MAX),
            FourCC: sys::NDIlib_FourCC_audio_type_e::NDIlib_FourCC_audio_type_FLTP,
            p_data: samples.as_mut_ptr().cast(),
            __bindgen_anon_1: sys::NDIlib_audio_frame_v3_t__bindgen_ty_1 {
                channel_stride_in_bytes: (frames * size_of::<f32>()) as i32,
            },
            p_metadata: null(),
            timestamp: 0,
        };
        // SAFETY: the synchronous SDK call borrows only the checked planar samples.
        unsafe { self.runtime.api.send_send_audio_v3.unwrap()(self.instance.as_ptr(), &frame) };
        Ok(())
    }
}

impl Drop for NdiSender {
    fn drop(&mut self) {
        // SAFETY: this is the sole owner and there are no outstanding SDK frame borrows.
        unsafe { self.runtime.api.send_destroy.unwrap()(self.instance.as_ptr()) };
    }
}

fn incomplete_send_api() -> NdiError {
    NdiError::Runtime("the NDI publication APIs are incomplete".into())
}
