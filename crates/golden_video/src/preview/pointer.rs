use super::{
    HostViewport,
    protocol::{CssRect, PreviewPointer},
};
use crate::plan::{OverlayShape, RegionGeometry};

pub(super) fn project(geometry: RegionGeometry, host: HostViewport, point: [f64; 2]) -> Option<PreviewPointer> {
    let scale = host.css_scale;
    let physical = [
        point[0] * scale + f64::from(host.webview_origin_x),
        point[1] * scale + f64::from(host.webview_origin_y),
    ];
    let target = [f64::from(host.client_width), f64::from(host.client_height)];
    let clip = geometry.overlay_clip;
    let clipped = physical[0] < f64::from(clip.x) * target[0]
        || physical[1] < f64::from(clip.y) * target[1]
        || physical[0] >= f64::from(clip.x + clip.width) * target[0]
        || physical[1] >= f64::from(clip.y + clip.height) * target[1];
    let destination = geometry.destination;
    let source = geometry.source_rect;
    let width = f64::from(destination.width) * target[0] / f64::from(source.width);
    let height = f64::from(destination.height) * target[1] / f64::from(source.height);
    let left = f64::from(destination.x) * target[0] - f64::from(source.x) * width;
    let top = f64::from(destination.y) * target[1] - f64::from(source.y) * height;
    let image = [(physical[0] - left) / width, (physical[1] - top) / height];
    if !image.iter().all(|value| (*value as f32).is_finite()) {
        return None;
    }
    let hit = geometry.overlays.iter().rev().find_map(|overlay| {
        if clipped {
            return None;
        }
        let id = overlay.hit.as_ref()?;
        let stroke = f64::from(overlay.width * geometry.overlay_scale) * 0.5;
        let inside = match overlay.shape {
            OverlayShape::Circle { center, radius } => {
                let dx = (image[0] - f64::from(center[0])) * width;
                let dy = (image[1] - f64::from(center[1])) * height;
                dx.hypot(dy) <= f64::from(radius * geometry.overlay_scale) + stroke
            }
            OverlayShape::Quad { corners } => {
                let mut inside = false;
                let mut edge_hit = false;
                for index in 0..4 {
                    let a = corners[index].map(f64::from);
                    let b = corners[(index + 1) % 4].map(f64::from);
                    if (a[1] > image[1]) != (b[1] > image[1])
                        && image[0] < a[0] + (image[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1])
                    {
                        inside = !inside;
                    }
                    let e = [(b[0] - a[0]) * width, (b[1] - a[1]) * height];
                    let p = [(image[0] - a[0]) * width, (image[1] - a[1]) * height];
                    let t = ((p[0] * e[0] + p[1] * e[1]) / (e[0] * e[0] + e[1] * e[1]).max(f64::MIN_POSITIVE))
                        .clamp(0.0, 1.0);
                    edge_hit |= (p[0] - e[0] * t).hypot(p[1] - e[1] * t) <= stroke;
                }
                inside || edge_hit
            }
        };
        inside.then(|| id.clone())
    });
    Some(PreviewPointer {
        rect: CssRect {
            x: (left - f64::from(host.webview_origin_x)) / scale,
            y: (top - f64::from(host.webview_origin_y)) / scale,
            width: width / scale,
            height: height / scale,
        },
        point: image.map(|value| value as f32),
        hit,
    })
}
