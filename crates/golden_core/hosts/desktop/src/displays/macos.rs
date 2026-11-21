//! AppKit display geometry and persistent ColorSync identities.

use std::{io, sync::mpsc, time::Duration};

use core_foundation::{
    base::TCFType,
    string::CFString,
    uuid::{CFUUID, CFUUIDCreateString, CFUUIDRef},
};
use core_graphics::display::CGDisplay;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSMainMenuWindowLevel, NSScreen, NSWindow};
use objc2_foundation::{NSNumber, ns_string};
use tauri::{AppHandle, Runtime, Window};

use super::NativeDisplay;
use crate::NativeOutputBounds;

#[link(name = "ColorSync", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayCreateUUIDFromDisplayID(display: u32) -> CFUUIDRef;
}

/// Snapshot active displays from the running desktop. Call on a preparation worker.
pub fn macos_desktop_displays<R: Runtime>(app: &AppHandle<R>) -> io::Result<Vec<NativeDisplay>> {
    on_main(app, |main| {
        let screens = NSScreen::screens(main);
        if screens.count() > 32 {
            return Err(io::Error::other("display catalog exceeds 32 active displays"));
        }
        let mut displays = Vec::with_capacity(screens.count());
        let mut identities = std::collections::HashSet::new();
        for (index, screen) in screens.iter().enumerate() {
            let (display, id) = screen_identity(&screen)?;
            if !identities.insert(id.clone()) {
                return Err(io::Error::other("active displays share a ColorSync UUID"));
            }
            let refresh_rate = display.display_mode().and_then(|mode| {
                let millihertz = (mode.refresh_rate() * 1000.0).round();
                (1.0..=f64::from(u32::MAX))
                    .contains(&millihertz)
                    .then(|| (millihertz as u32, 1000))
            });
            displays.push(NativeDisplay {
                id,
                name: screen.localizedName().to_string(),
                edid: None,
                index: (index + 1) as u32,
                primary: display.is_main(),
                internal: display.is_builtin().then_some(true),
                refresh_rate,
                bounds: bounds(&screen, display)?,
            });
        }
        Ok(displays)
    })
}

pub(crate) fn place_fullscreen<R: Runtime>(
    app: &AppHandle<R>,
    window: Window<R>,
    identity: String,
    expected: NativeOutputBounds,
) -> io::Result<()> {
    on_main(app, move |main| {
        let mut selected = None;
        for screen in NSScreen::screens(main).iter() {
            let (display, id) = screen_identity(&screen)?;
            if id != identity {
                continue;
            }
            if bounds(&screen, display)? != expected || selected.replace(screen.frame()).is_some() {
                return Err(io::Error::other("the selected display's geometry or identity changed"));
            }
        }
        let frame = selected
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "the selected display changed before placement"))?;
        let handle = window.ns_window().map_err(io::Error::other)?;
        // SAFETY: Tauri owns this NSWindow. The retained Tauri window and main-thread token
        // keep the borrowed pointer valid for this synchronous placement.
        let native = unsafe { handle.cast::<NSWindow>().as_ref() }
            .ok_or_else(|| io::Error::other("missing AppKit output window"))?;
        native.setFrame_display(frame, false);
        native.setMovable(false);
        native.setLevel(NSMainMenuWindowLevel + 1);
        if native.frame() != frame {
            return Err(io::Error::other("AppKit constrained the fullscreen output frame"));
        }
        Ok(())
    })
}

fn screen_identity(screen: &NSScreen) -> io::Result<(CGDisplay, String)> {
    let number = screen
        .deviceDescription()
        .objectForKey(ns_string!("NSScreenNumber"))
        .and_then(|value| value.downcast::<NSNumber>().ok())
        .ok_or_else(|| io::Error::other("NSScreen has no display number"))?;
    let display = CGDisplay::new(number.unsignedIntValue());
    // SAFETY: ColorSync returns an owned CFUUID for the WindowServer display number.
    let uuid = unsafe { CGDisplayCreateUUIDFromDisplayID(display.id) };
    if uuid.is_null() {
        return Err(io::Error::other("ColorSync has no UUID for the active display"));
    }
    let uuid = unsafe { CFUUID::wrap_under_create_rule(uuid) };
    // SAFETY: the allocator is the default and the UUID is retained through the call.
    let string = unsafe { CFUUIDCreateString(std::ptr::null(), uuid.as_concrete_TypeRef()) };
    if string.is_null() {
        return Err(io::Error::other("cannot format the display UUID"));
    }
    let string = unsafe { CFString::wrap_under_create_rule(string) };
    Ok((display, format!("macos:{string}")))
}

fn bounds(screen: &NSScreen, display: CGDisplay) -> io::Result<NativeOutputBounds> {
    let backing = screen.convertRectToBacking(screen.frame());
    let position = display.bounds().origin;
    let scale = screen.backingScaleFactor();
    NativeOutputBounds::new(
        (position.x * scale).round() as i32,
        (position.y * scale).round() as i32,
        backing.size.width.round() as u32,
        backing.size.height.round() as u32,
    )
}

fn on_main<R, T>(
    app: &AppHandle<R>,
    run: impl FnOnce(MainThreadMarker) -> io::Result<T> + Send + 'static,
) -> io::Result<T>
where
    R: Runtime,
    T: Send + 'static,
{
    if let Some(main) = MainThreadMarker::new() {
        return run(main);
    }
    let (send, receive) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let result = MainThreadMarker::new()
            .ok_or_else(|| io::Error::other("AppKit operation requires the main thread"))
            .and_then(run);
        let _ = send.send(result);
    })
    .map_err(io::Error::other)?;
    receive.recv_timeout(Duration::from_secs(2)).map_err(io::Error::other)?
}
