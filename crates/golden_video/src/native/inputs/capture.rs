use super::{
    receiver::{Received, Receiver},
    video,
};
use crate::{
    TextureDescriptor,
    native::{device::Device, texture::Texture},
};
use windows::core::Result;

pub(in crate::native) enum Capture {
    Rgb(Texture),
    Video(video::Image),
}
impl Capture {
    pub fn new(device: &Device, descriptor: TextureDescriptor, native: u32) -> Result<Self> {
        match video::Format::from_native(native) {
            Some(format) => video::Image::new(device, descriptor, format).map(Self::Video),
            None => Texture::new(&device.device, descriptor).map(Self::Rgb),
        }
    }
    #[cfg(feature = "qualification")]
    pub fn rgb(&self) -> Option<&Texture> {
        match self {
            Self::Rgb(image) => Some(image),
            Self::Video(_) => None,
        }
    }
    pub fn receive(&self, receiver: &mut Receiver) -> Result<Received> {
        match self {
            Self::Rgb(image) => receiver.receive(image),
            Self::Video(image) => receiver.receive_resource(&image.texture, image.descriptor, None),
        }
    }
}
