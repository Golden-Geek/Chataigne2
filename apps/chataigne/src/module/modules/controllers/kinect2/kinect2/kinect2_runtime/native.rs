use super::*;
#[cfg(windows)]
mod windows;
pub(super) struct Native {
    #[cfg(windows)]
    inner: windows::PlatformKinect2Runtime,
    last_available: Option<bool>,
}
pub(super) fn create() -> Result<Native, String> {
    if cfg!(test) {
        return Err("Native Kinect endpoints are disabled in test builds".into());
    }
    #[cfg(windows)]
    {
        Ok(Native {
            inner: windows::PlatformKinect2Runtime::create()?,
            last_available: None,
        })
    }
    #[cfg(not(windows))]
    {
        Err("Kinect not supported on this OS.".into())
    }
}
impl Native {
    fn sample(&mut self) -> Result<KinectRuntimePoll, String> {
        #[cfg(windows)]
        {
            self.inner.poll()
        }
        #[cfg(not(windows))]
        {
            Err("Kinect not supported on this OS.".into())
        }
    }
}
impl Provider for Native {
    fn poll(&mut self) -> Result<Vec<KinectRuntimePoll>, String> {
        let row = self.sample()?;
        if row.frame.is_none() && self.last_available == Some(row.sensor_available) {
            return Ok(Vec::new());
        }
        self.last_available = Some(row.sensor_available);
        Ok(vec![row])
    }
}
