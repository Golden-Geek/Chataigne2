//! GTK-owned Wayland output metadata and compositor-directed fullscreen placement.

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    io,
    rc::Rc,
    sync::mpsc,
    time::Duration,
};

use gtk::{
    gdk,
    glib::{
        GString, Propagation,
        translate::{ToGlibPtr, from_glib_full},
    },
    prelude::*,
};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Runtime, Window};

use super::NativeDisplay;
use crate::NativeOutputBounds;

pub(crate) fn desktop_displays<R: Runtime>(app: &AppHandle<R>) -> io::Result<Vec<NativeDisplay>> {
    on_main(app, || {
        let display = gdk::Display::default().ok_or_else(|| io::Error::other("missing Wayland display"))?;
        Ok(outputs(&display)?.into_iter().map(|(_, display)| display).collect())
    })
}

pub(crate) fn place_fullscreen<R: Runtime>(
    app: &AppHandle<R>,
    window: Window<R>,
    identity: String,
    expected: NativeOutputBounds,
) -> io::Result<()> {
    let (painted, first_paint) = mpsc::sync_channel(1);
    on_main(app, move || {
        let widget = window.gtk_window().map_err(io::Error::other)?;
        let display = WidgetExt::display(&widget);
        let (index, _) = outputs(&display)?
            .into_iter()
            .find(|(_, display)| display.id == identity && display.bounds == expected)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "the selected Wayland output changed before placement",
                )
            })?;
        let screen = WidgetExt::screen(&widget).ok_or_else(|| io::Error::other("missing Wayland screen"))?;
        let monitor = display
            .monitor(index)
            .ok_or_else(|| io::Error::other("missing selected Wayland output"))?;
        let logical = monitor.geometry();
        // A larger first GTK buffer can be rejected even when xdg-shell has
        // acknowledged fullscreen. Start at the compositor's logical size.
        widget.set_default_size(logical.width(), logical.height());
        widget.resize(logical.width(), logical.height());
        widget.fullscreen_on_monitor(&screen, index);
        widget.realize();
        let clock = widget
            .frame_clock()
            .ok_or_else(|| io::Error::other("missing Wayland frame clock"))?;
        let scale = monitor.scale_factor();
        let drew_buffer = Rc::new(Cell::new(false));
        let draw_ready = drew_buffer.clone();
        let draw = widget.connect_draw(move |widget, cairo| {
            if widget.allocated_width() != logical.width()
                || widget.allocated_height() != logical.height()
                || widget.scale_factor() != scale
            {
                return Propagation::Proceed;
            }
            cairo.set_source_rgb(0.0, 0.0, 0.0);
            if cairo.paint().is_ok() {
                draw_ready.set(true);
            }
            Propagation::Stop
        });
        let first = Rc::new(RefCell::new(Some((painted, draw, None))));
        let callback = first.clone();
        let weak_widget = widget.downgrade();
        let handler = clock.connect_after_paint(move |clock| {
            // A clock phase can finish without drawing this widget. Only release the worker
            // after GDK's after-paint handler has committed a buffer at the selected scale.
            if !drew_buffer.replace(false) {
                return;
            }
            let ready = callback.borrow_mut().take();
            if let Some((painted, draw, handler)) = ready {
                if let Some(widget) = weak_widget.upgrade() {
                    widget.disconnect(draw);
                }
                if let Some(handler) = handler {
                    clock.disconnect(handler);
                }
                let _ = painted.try_send(());
            }
        });
        first.borrow_mut().as_mut().expect("first paint is pending").2 = Some(handler);
        widget.show();
        Ok(())
    })?;
    // The render worker borrows the surface only after GTK's initial configured
    // draw commits the device buffer scale.
    first_paint.recv_timeout(Duration::from_secs(2)).map_err(|error| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            format!("Wayland fullscreen did not paint: {error}"),
        )
    })
}

fn outputs(display: &gdk::Display) -> io::Result<Vec<(i32, NativeDisplay)>> {
    let count = display.n_monitors();
    if count > 32 {
        return Err(io::Error::other("display catalog exceeds 32 active outputs"));
    }
    let screen = display.default_screen();
    let mut ports = HashSet::new();
    let mut outputs = Vec::new();
    for index in 0..count {
        let Some(monitor) = display.monitor(index) else {
            continue;
        };
        // GTK 3 exposes the compositor name through the plug-name API. The model is
        // hardware metadata and cannot distinguish identical monitors on different ports.
        // SAFETY: the GTK thread owns a live screen and monitor for this index. GDK returns
        // an optional newly allocated string, transferred to GString.
        let connector: Option<GString> = unsafe {
            from_glib_full(gdk::ffi::gdk_screen_get_monitor_plug_name(
                screen.to_glib_none().0,
                index,
            ))
        };
        let Some(connector) = connector.filter(|name| !name.is_empty()) else {
            continue;
        };
        if !ports.insert(connector.clone()) {
            return Err(io::Error::other("active Wayland outputs share a connector name"));
        }
        let geometry = monitor.geometry();
        let scale = monitor.scale_factor();
        let pixels = |logical: i32| {
            logical
                .checked_mul(scale)
                .and_then(|pixels| u32::try_from(pixels).ok())
                .filter(|pixels| *pixels > 0)
        };
        let (Some(width), Some(height)) = (pixels(geometry.width()), pixels(geometry.height())) else {
            continue;
        };
        let manufacturer = monitor.manufacturer().unwrap_or_default();
        let model = monitor.model().unwrap_or_default();
        let mut fingerprint = Sha256::new();
        for field in [&manufacturer, &model] {
            fingerprint.update((field.len() as u64).to_le_bytes());
            fingerprint.update(field.as_bytes());
        }
        let mut size_mm = [monitor.width_mm(), monitor.height_mm()];
        size_mm.sort_unstable();
        for size in size_mm {
            fingerprint.update(size.to_le_bytes());
        }
        let name = if model.is_empty() || model == connector {
            connector.to_string()
        } else {
            format!("{model} ({connector})")
        };
        outputs.push((
            index,
            NativeDisplay {
                id: format!("wayland:{connector}:{:x}", fingerprint.finalize()),
                name,
                edid: None,
                index: (index + 1) as u32,
                primary: monitor.is_primary(),
                internal: ["eDP", "LVDS", "DSI"]
                    .iter()
                    .any(|prefix| connector.starts_with(prefix))
                    .then_some(true),
                refresh_rate: u32::try_from(monitor.refresh_rate())
                    .ok()
                    .filter(|rate| *rate != 0)
                    .map(|rate| (rate, 1000)),
                // The compositor exclusively controls the output's location. The size is the
                // buffer extent in GDK device pixels, including integer display scaling.
                bounds: NativeOutputBounds::new(0, 0, width, height)?,
            },
        ));
    }
    Ok(outputs)
}

fn on_main<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    operation: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> io::Result<T> {
    let (send, receive) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = send.send(operation());
    })
    .map_err(io::Error::other)?;
    receive.recv_timeout(Duration::from_secs(2)).map_err(io::Error::other)?
}
