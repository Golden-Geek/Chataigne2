//! XRandR output metadata, queried on the native preparation worker.

use std::{collections::HashSet, io};

use sha2::{Digest, Sha256};
use x11rb::{
    connection::Connection as _,
    protocol::{
        randr::{Connection, ConnectionExt as _, ModeFlag, ModeInfo, SetConfig},
        xproto::{AtomEnum, ConnectionExt as _},
    },
};

use super::{NativeDisplay, metadata::canonical_edid};
use crate::NativeOutputBounds;

/// Enumerate addressable outputs on the host's active X11 screen.
pub fn x11_desktop_displays() -> io::Result<Vec<NativeDisplay>> {
    let (connection, screen) = x11rb::connect(None).map_err(io::Error::other)?;
    let root = connection.setup().roots[screen].root;
    let edid = connection
        .intern_atom(true, b"EDID")
        .map_err(io::Error::other)?
        .reply()
        .map_err(io::Error::other)?
        .atom;
    let primary = connection
        .randr_get_output_primary(root)
        .map_err(io::Error::other)?
        .reply()
        .map_err(io::Error::other)?
        .output;
    let resources = connection
        .randr_get_screen_resources_current(root)
        .map_err(io::Error::other)?
        .reply()
        .map_err(io::Error::other)?;
    let mut displays = Vec::new();
    for output in resources.outputs {
        let info = connection
            .randr_get_output_info(output, resources.config_timestamp)
            .map_err(io::Error::other)?
            .reply()
            .map_err(io::Error::other)?;
        if info.status != SetConfig::SUCCESS {
            return Err(io::Error::other("XRandR outputs changed during enumeration"));
        }
        if info.connection != Connection::CONNECTED || info.crtc == 0 {
            continue;
        }
        let crtc = connection
            .randr_get_crtc_info(info.crtc, resources.config_timestamp)
            .map_err(io::Error::other)?
            .reply()
            .map_err(io::Error::other)?;
        if crtc.status != SetConfig::SUCCESS {
            return Err(io::Error::other("XRandR geometry changed during enumeration"));
        }
        if crtc.mode == 0 {
            continue;
        }
        let name = String::from_utf8(info.name).map_err(io::Error::other)?;
        let edid = if edid == 0 {
            None
        } else {
            let property = connection
                .randr_get_output_property(output, edid, AtomEnum::ANY, 0, 32, false, false)
                .map_err(io::Error::other)?
                .reply()
                .map_err(io::Error::other)?;
            (property.format == 8).then(|| canonical_edid(&property.data)).flatten()
        };
        // The connector disambiguates panels that report identical or missing EDIDs.
        let id = match &edid {
            Some(edid) => format!("x11:{name}:{:x}", Sha256::digest(edid.as_bytes())),
            None => format!("x11:{name}"),
        };
        let bounds = NativeOutputBounds::new(
            i32::from(crtc.x),
            i32::from(crtc.y),
            u32::from(crtc.width),
            u32::from(crtc.height),
        )?;
        let refresh_rate = resources
            .modes
            .iter()
            .find(|mode| mode.id == crtc.mode)
            .and_then(refresh_rate);
        displays.push(NativeDisplay {
            id,
            internal: ["eDP", "LVDS", "DSI"]
                .iter()
                .any(|prefix| name.starts_with(prefix))
                .then_some(true),
            name,
            edid,
            index: 0,
            primary: output == primary || (primary == 0 && crtc.x == 0 && crtc.y == 0),
            refresh_rate,
            bounds,
        });
    }
    displays.sort_by(|left, right| {
        (!left.primary, left.bounds.top(), left.bounds.left(), &left.id).cmp(&(
            !right.primary,
            right.bounds.top(),
            right.bounds.left(),
            &right.id,
        ))
    });
    // Mirrored connectors expose the same pixel region and cannot show independent windows.
    let mut regions = HashSet::new();
    displays.retain(|display| {
        let bounds = display.bounds;
        regions.insert((bounds.left(), bounds.top(), bounds.width(), bounds.height()))
    });
    if displays.len() > 32 {
        return Err(io::Error::other("display catalog exceeds 32 active displays"));
    }
    for (index, display) in displays.iter_mut().enumerate() {
        display.index = (index + 1) as u32;
    }
    Ok(displays)
}

fn refresh_rate(mode: &ModeInfo) -> Option<(u32, u32)> {
    let mut numerator = mode.dot_clock;
    let mut denominator = u32::from(mode.htotal) * u32::from(mode.vtotal);
    if mode.mode_flags.contains(ModeFlag::INTERLACE) {
        numerator = numerator.checked_mul(2)?;
    }
    if mode.mode_flags.contains(ModeFlag::DOUBLE_SCAN) {
        denominator = denominator.checked_mul(2)?;
    }
    (numerator > 0 && denominator > 0).then_some((numerator, denominator))
}
