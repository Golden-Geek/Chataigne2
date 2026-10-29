use super::{
    ResolvedView,
    protocol::{CssRect, PreviewRejection, PreviewView},
};
use crate::plan::UnitRect;

/// Authenticated host observations. CSS scale includes WebView rasterization and zoom; clients
/// cannot provide it. Native WebView bounds are relative to the borrowed window's client area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostViewport {
    /// Actual native client width in physical pixels, including zero while unavailable.
    pub client_width: u32,
    /// Actual native client height in physical pixels.
    pub client_height: u32,
    /// Physical WebView left edge relative to the native client origin.
    pub webview_origin_x: i32,
    /// Physical WebView top edge relative to the native client origin.
    pub webview_origin_y: i32,
    /// Actual physical WebView width after the host's BoundsMode conversion.
    pub webview_width: u32,
    /// Actual physical WebView height after the host's BoundsMode conversion.
    pub webview_height: u32,
    /// Authenticated CSS-to-physical scale, including WebView rasterization and HTML zoom.
    pub css_scale: f64,
}

impl HostViewport {
    pub(super) fn valid(self) -> bool {
        self.css_scale.is_finite()
            && (0.05..=64.).contains(&self.css_scale)
            && self.client_width <= 16384
            && self.client_height <= 16384
            && self.webview_width <= 16384
            && self.webview_height <= 16384
            && self.webview_origin_x.unsigned_abs() <= 1_000_000
            && self.webview_origin_y.unsigned_abs() <= 1_000_000
    }
}

fn valid(rect: CssRect) -> bool {
    [rect.x, rect.y, rect.width, rect.height]
        .iter()
        .all(|value| value.is_finite())
        && rect.x.abs() <= 1_000_000.
        && rect.y.abs() <= 1_000_000.
        && (0.0..=1_000_000.).contains(&rect.width)
        && (0.0..=1_000_000.).contains(&rect.height)
}

fn unit(left: f64, top: f64, right: f64, bottom: f64) -> Option<UnitRect> {
    let x = left.clamp(0., 1.) as f32;
    let y = top.clamp(0., 1.) as f32;
    let width = ((right.clamp(0., 1.) as f32) - x).min(1. - x);
    let height = ((bottom.clamp(0., 1.) as f32) - y).min(1. - y);
    let rect = UnitRect { x, y, width, height };
    rect.is_valid().then_some(rect)
}

pub(super) fn resolve(view: PreviewView, host: HostViewport) -> Result<Option<ResolvedView>, PreviewRejection> {
    if !host.valid() || !valid(view.rect) || !valid(view.clip) {
        return Err(PreviewRejection::Geometry);
    }
    if view.source.is_empty() || view.source.len() > 255 || view.source.contains('\0') {
        return Err(PreviewRejection::Source);
    }
    if !view.visible
        || view.rect.width == 0.
        || view.rect.height == 0.
        || host.client_width == 0
        || host.client_height == 0
    {
        return Ok(None);
    }
    let scale = host.css_scale;
    let ox = f64::from(host.webview_origin_x);
    let oy = f64::from(host.webview_origin_y);
    let left = ox + view.rect.x * scale;
    let top = oy + view.rect.y * scale;
    let width = view.rect.width * scale;
    let height = view.rect.height * scale;
    let clipped_left = left.max(ox + view.clip.x * scale).max(ox).max(0.);
    let clipped_top = top.max(oy + view.clip.y * scale).max(oy).max(0.);
    let clipped_right = (left + width)
        .min(ox + (view.clip.x + view.clip.width) * scale)
        .min(ox + f64::from(host.webview_width))
        .min(f64::from(host.client_width));
    let clipped_bottom = (top + height)
        .min(oy + (view.clip.y + view.clip.height) * scale)
        .min(oy + f64::from(host.webview_height))
        .min(f64::from(host.client_height));
    if clipped_left >= clipped_right || clipped_top >= clipped_bottom {
        return Ok(None);
    }
    let destination = unit(
        clipped_left / f64::from(host.client_width),
        clipped_top / f64::from(host.client_height),
        clipped_right / f64::from(host.client_width),
        clipped_bottom / f64::from(host.client_height),
    );
    let source_rect = unit(
        (clipped_left - left) / width,
        (clipped_top - top) / height,
        (clipped_right - left) / width,
        (clipped_bottom - top) / height,
    );
    Ok(destination
        .zip(source_rect)
        .map(|(destination, source_rect)| ResolvedView {
            id: view.id,
            // Typed callers can supply spare String capacity that never belonged to the wire.
            // Retain only the admitted source key, not that unbounded allocation headroom.
            source: view.source.into_boxed_str().into_string(),
            destination,
            source_rect,
        }))
}
