//! Authored output policy resolves against current physical routing, never persisted OS rate hints.
use super::{
    protocol::{DisplayInfo, PhysicalRectangle, ScreenFrameRate},
    rendering::ResolvedTarget,
};
use golden_time::RationalRate;

pub(super) fn resolve(policy: ScreenFrameRate, target: &ResolvedTarget, displays: &[DisplayInfo]) -> RationalRate {
    if let ScreenFrameRate::Explicit { fps } = policy {
        return ScreenFrameRate::explicit_rate(fps).expect("validated Screen frame rate");
    }
    let display = match target {
        ResolvedTarget::Fullscreen { display, .. } => displays.iter().find(|entry| entry.id == *display),
        ResolvedTarget::Rectangle(bounds) => displays
            .iter()
            .filter_map(|display| {
                let area = intersection(*bounds, display.bounds);
                (area != 0).then_some((display, area))
            })
            .max_by(|(a, area_a), (b, area_b)| {
                area_a
                    .cmp(area_b)
                    .then(a.primary.cmp(&b.primary))
                    .then_with(|| b.id.cmp(&a.id))
            })
            .map(|(display, _)| display),
        ResolvedTarget::Spout { .. } => None,
    };
    display
        .and_then(|display| display.refresh_rate)
        .and_then(|rate| RationalRate::new(rate.numerator, rate.denominator).ok())
        .unwrap_or_else(|| RationalRate::new(60, 1).unwrap())
}

fn intersection(a: PhysicalRectangle, b: PhysicalRectangle) -> u64 {
    let width = (i64::from(a.left) + i64::from(a.extent.width())).min(i64::from(b.left) + i64::from(b.extent.width()))
        - i64::from(a.left).max(i64::from(b.left));
    let height = (i64::from(a.top) + i64::from(a.extent.height())).min(i64::from(b.top) + i64::from(b.extent.height()))
        - i64::from(a.top).max(i64::from(b.top));
    width.max(0) as u64 * height.max(0) as u64
}

#[cfg(test)]
mod tests;
