use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use tauri::{Manager, Runtime, WebviewWindow};

use super::host::PresentationHost;

pub(super) const MAX_CONTROL_BYTES: usize = 64 * 1024;
const MAX_PENDING_CONTROLS: usize = 8;
pub(super) type ControlHandler =
    dyn Fn(&str, NativePresentationViewport) -> Result<String, String> + Send + Sync + 'static;

/// Fresh host-owned geometry, queried on the WebView UI thread. This is internal host metadata,
/// not a wire declaration; preview protocol types belong to the consuming reusable subsystem.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativePresentationViewport {
    /// Actual HWND client width in physical pixels, including zero during minimization.
    pub physical_width: u32,
    /// Actual HWND client height in physical pixels.
    pub physical_height: u32,
    /// WebView left edge relative to the HWND client, in physical pixels.
    pub webview_origin_x: i32,
    /// WebView top edge relative to the HWND client, in physical pixels.
    pub webview_origin_y: i32,
    /// Actual WebView physical width after its BoundsMode conversion.
    pub webview_width: u32,
    /// Actual WebView physical height after its BoundsMode conversion.
    pub webview_height: u32,
    /// WebView rasterization scale multiplied by HTML zoom, never frontend devicePixelRatio.
    pub css_scale: f64,
}

#[derive(Default)]
pub(super) struct ControlState {
    endpoint: Option<ControlEndpoint>,
}

struct ControlEndpoint {
    window: String,
    current: Arc<AtomicBool>,
    handler: Arc<ControlHandler>,
    pending: Arc<AtomicUsize>,
}

pub(super) struct ControlTicket {
    current: Arc<AtomicBool>,
    handler: Arc<ControlHandler>,
    _permit: ControlPermit,
}

struct ControlPermit(Arc<AtomicUsize>);
impl Drop for ControlPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl ControlState {
    pub fn attach(&mut self, window: &str, handler: Option<Arc<ControlHandler>>) {
        self.close();
        self.endpoint = handler.map(|handler| ControlEndpoint {
            window: window.into(),
            current: Arc::new(AtomicBool::new(true)),
            handler,
            pending: Arc::new(AtomicUsize::new(0)),
        });
    }
    pub fn close(&mut self) {
        if let Some(endpoint) = self.endpoint.take() {
            endpoint.current.store(false, Ordering::Release);
        }
    }
    pub fn admit(&self, window: &str, request: &str) -> Result<ControlTicket, String> {
        if request.len() > MAX_CONTROL_BYTES {
            return Err("native presentation request exceeds 64 KiB".into());
        }
        let endpoint = self
            .endpoint
            .as_ref()
            .ok_or("native presentation control is unavailable")?;
        if endpoint.window != window {
            return Err("native presentation belongs to another window".into());
        }
        if !endpoint.current.load(Ordering::Acquire) {
            return Err("native presentation is retiring".into());
        }
        endpoint
            .pending
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                (pending < MAX_PENDING_CONTROLS).then_some(pending + 1)
            })
            .map_err(|_| "native presentation control capacity is exhausted")?;
        Ok(ControlTicket {
            current: endpoint.current.clone(),
            handler: endpoint.handler.clone(),
            _permit: ControlPermit(endpoint.pending.clone()),
        })
    }
}

impl ControlTicket {
    pub fn dispatch(&self, request: &str, viewport: NativePresentationViewport) -> Result<String, String> {
        if !self.current.load(Ordering::Acquire) {
            return Err("native presentation is retiring".into());
        }
        let response = (self.handler)(request, viewport);
        // Admission can precede concurrent worker retirement; never acknowledge a now-stale view.
        if !self.current.load(Ordering::Acquire) {
            return Err("native presentation is retiring".into());
        }
        match response {
            Ok(response) if response.len() <= MAX_CONTROL_BYTES => Ok(response),
            Err(error) if error.len() <= MAX_CONTROL_BYTES => Err(error),
            _ => Err("native presentation response exceeds 64 KiB".into()),
        }
    }
}

pub(crate) async fn invoke<R: Runtime>(window: WebviewWindow<R>, request: String) -> Result<String, String> {
    let host = window
        .try_state::<Arc<PresentationHost>>()
        .map(|state| state.inner().clone())
        .ok_or("native presentation control is unavailable")?;
    let ticket = host.admit_control(window.label(), &request)?;
    invoke_scoped(window, request, ticket).await
}

#[cfg(target_os = "windows")]
async fn invoke_scoped<R: Runtime>(
    window: WebviewWindow<R>,
    request: String,
    ticket: ControlTicket,
) -> Result<String, String> {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_BOUNDS_MODE_USE_RASTERIZATION_SCALE, COREWEBVIEW2_BOUNDS_MODE_USE_RAW_PIXELS,
        ICoreWebView2Controller3,
    };
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
    use windows::core::Interface;

    let hwnd = window.hwnd().map_err(|error| error.to_string())?.0 as isize;
    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    window
        .with_webview(move |webview| {
            let result = (|| {
                let controller = webview.controller();
                let controller3 = controller
                    .cast::<ICoreWebView2Controller3>()
                    .map_err(|error| error.to_string())?;
                let mut bounds = RECT::default();
                let mut client = RECT::default();
                let mut raster = 0.0;
                let mut zoom = 0.0;
                let mut mode = COREWEBVIEW2_BOUNDS_MODE_USE_RAW_PIXELS;
                // SAFETY: all WebView COM access is scoped to its UI thread; only scalar metrics
                // escape. The host owns this HWND and invalidates control before window retirement.
                unsafe {
                    controller.Bounds(&mut bounds).map_err(|error| error.to_string())?;
                    controller.ZoomFactor(&mut zoom).map_err(|error| error.to_string())?;
                    controller3
                        .RasterizationScale(&mut raster)
                        .map_err(|error| error.to_string())?;
                    controller3.BoundsMode(&mut mode).map_err(|error| error.to_string())?;
                    GetClientRect(HWND(hwnd as *mut _), &mut client).map_err(|error| error.to_string())?;
                }
                let raster_bounds = if mode == COREWEBVIEW2_BOUNDS_MODE_USE_RAW_PIXELS {
                    false
                } else if mode == COREWEBVIEW2_BOUNDS_MODE_USE_RASTERIZATION_SCALE {
                    true
                } else {
                    return Err("unsupported WebView bounds mode".into());
                };
                let viewport = from_metrics(
                    [client.right - client.left, client.bottom - client.top],
                    [bounds.left, bounds.top, bounds.right, bounds.bottom],
                    raster,
                    zoom,
                    raster_bounds,
                )?;
                ticket.dispatch(&request, viewport)
            })();
            // Capacity one, exactly one producer: completing never waits on the UI thread.
            let _ = sender.try_send(result);
        })
        .map_err(|error| error.to_string())?;
    receiver
        .recv()
        .await
        .ok_or_else(|| "native presentation UI callback ended without a response".to_owned())?
}

#[cfg(not(target_os = "windows"))]
async fn invoke_scoped<R: Runtime>(
    _window: WebviewWindow<R>,
    _request: String,
    _ticket: ControlTicket,
) -> Result<String, String> {
    Err("native presentation metrics are unavailable on this platform".into())
}

fn from_metrics(
    client: [i32; 2],
    bounds: [i32; 4],
    raster: f64,
    zoom: f64,
    raster_bounds: bool,
) -> Result<NativePresentationViewport, String> {
    let css_scale = raster * zoom;
    if !raster.is_finite() || raster <= 0.0 || !zoom.is_finite() || zoom <= 0.0 || !css_scale.is_finite() {
        return Err("invalid WebView scale metrics".into());
    }
    let factor = if raster_bounds { raster } else { 1.0 };
    let edge = |value: i32, upper: bool| -> Result<i32, String> {
        let scaled = f64::from(value) * factor;
        let rounded = if upper { scaled.ceil() } else { scaled.floor() };
        if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
            return Err("WebView bounds exceed physical coordinate range".into());
        }
        Ok(rounded as i32)
    };
    if bounds[2] < bounds[0] || bounds[3] < bounds[1] {
        return Err("invalid WebView bounds".into());
    }
    let left = edge(bounds[0], false)?;
    let top = edge(bounds[1], false)?;
    let right = edge(bounds[2], true)?;
    let bottom = edge(bounds[3], true)?;
    Ok(NativePresentationViewport {
        physical_width: u32::try_from(client[0]).map_err(|_| "invalid native client width")?,
        physical_height: u32::try_from(client[1]).map_err(|_| "invalid native client height")?,
        webview_origin_x: left,
        webview_origin_y: top,
        webview_width: u32::try_from(i64::from(right) - i64::from(left))
            .map_err(|_| "invalid physical WebView width")?,
        webview_height: u32::try_from(i64::from(bottom) - i64::from(top))
            .map_err(|_| "invalid physical WebView height")?,
        css_scale,
    })
}

#[cfg(test)]
mod tests;
