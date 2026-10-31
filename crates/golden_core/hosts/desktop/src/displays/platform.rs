use super::*;
use windows::Win32::{
    Devices::Display::*,
    Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS},
};

pub(super) fn enumerate() -> io::Result<Vec<NativeDisplay>> {
    for _ in 0..3 {
        let (mut path_count, mut mode_count) = (0, 0);
        // SAFETY: fixed flags and owned bounded arrays; the API returns actual initialized counts.
        unsafe {
            let status = GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count);
            if status != ERROR_SUCCESS {
                return Err(io::Error::from_raw_os_error(status.0 as i32));
            }
            if path_count > 32 || mode_count > 128 {
                return Err(io::Error::other("native display catalog exceeds 32 paths/128 modes"));
            }
            if path_count == 0 {
                return Ok(Vec::new());
            }
            let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
            let status = QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut path_count,
                paths.as_mut_ptr(),
                &mut mode_count,
                modes.as_mut_ptr(),
                None,
            );
            if status == ERROR_INSUFFICIENT_BUFFER {
                continue;
            }
            if status != ERROR_SUCCESS {
                return Err(io::Error::from_raw_os_error(status.0 as i32));
            }
            paths.truncate(path_count as usize);
            modes.truncate(mode_count as usize);
            let mut displays = Vec::with_capacity(paths.len());
            for path in paths {
                let source = modes
                    .iter()
                    .find(|mode| {
                        mode.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE
                            && mode.id == path.sourceInfo.id
                            && mode.adapterId.LowPart == path.sourceInfo.adapterId.LowPart
                            && mode.adapterId.HighPart == path.sourceInfo.adapterId.HighPart
                    })
                    .ok_or_else(|| io::Error::other("active display has no source mode"))?;
                let source = source.Anonymous.sourceMode; // Checked infoType selects this union member.
                let mut name = DISPLAYCONFIG_TARGET_DEVICE_NAME {
                    header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                        r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                        size: size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                        adapterId: path.targetInfo.adapterId,
                        id: path.targetInfo.id,
                    },
                    ..Default::default()
                };
                let status = DisplayConfigGetDeviceInfo(&mut name.header);
                if status != 0 {
                    return Err(io::Error::from_raw_os_error(status));
                }
                let id = String::from_utf16(terminated(&name.monitorDevicePath))
                    .map_err(io::Error::other)?
                    .to_ascii_lowercase();
                if id.is_empty() {
                    return Err(io::Error::other("active display has no stable device path"));
                }
                displays.push(NativeDisplay {
                    id,
                    name: String::from_utf16_lossy(terminated(&name.monitorFriendlyDeviceName)),
                    bounds: NativeOutputBounds::new(source.position.x, source.position.y, source.width, source.height)?,
                });
            }
            displays.sort_by(|left, right| left.id.cmp(&right.id));
            if displays.windows(2).any(|pair| pair[0].id == pair[1].id) {
                return Err(io::Error::other("duplicate native display identity"));
            }
            return Ok(displays);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "display topology changed during bounded enumeration",
    ))
}

fn terminated(value: &[u16]) -> &[u16] {
    &value[..value.iter().position(|value| *value == 0).unwrap_or(value.len())]
}
