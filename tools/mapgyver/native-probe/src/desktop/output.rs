//! Fixed-scene output placement; all HWND operations run on the creating UI thread.

use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONULL, MONITORINFOEXW, MonitorFromWindow,
};
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos};

use crate::ProbeResult;
use crate::d3d::OutputInfo;
use crate::dpi::PhysicalDpiScope;

pub(super) struct Target {
    device_name: String,
    desktop_rect: [i32; 4],
    extent: [u32; 2],
}

impl Target {
    pub(super) fn select(adapter: u32, extent: [u32; 2]) -> ProbeResult<Self> {
        let adapter = crate::d3d::adapter_info(adapter)?;
        let output = choose(&adapter.outputs, extent).map_err(|error| {
            format!(
                "native output fixture on adapter {} ({}): {error}",
                adapter.index, adapter.luid
            )
        })?;
        Ok(Self {
            device_name: output.device_name.clone(),
            desktop_rect: output.desktop_rect,
            extent,
        })
    }

    pub(super) fn place(&self, window: HWND) -> ProbeResult<()> {
        let _dpi = PhysicalDpiScope::enter()?;
        let [left, top, _, _] = self.desktop_rect;
        let [width, height] = self.extent;
        let width = i32::try_from(width)?;
        let height = i32::try_from(height)?;
        // SAFETY: setup owns this hidden, undecorated, shadow-free HWND on its UI thread.
        // Relocate first so Tao's synchronous WM_DPICHANGED handling finishes before the
        // final physical-size authority. Neither call queues asynchronous placement.
        unsafe {
            SetWindowPos(
                window,
                None,
                left,
                top,
                0,
                0,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE,
            )?;
            SetWindowPos(window, None, left, top, width, height, SWP_NOZORDER | SWP_NOACTIVATE)?;
        }
        Ok(())
    }

    pub(super) fn verify(&self, window: HWND) -> ProbeResult<()> {
        let _dpi = PhysicalDpiScope::enter()?;
        let mut client = RECT::default();
        let mut origin = POINT::default();
        // SAFETY: these are read-only observations of the same owned HWND, after show and
        // before the renderer or any fixture/consumer GPU worker starts.
        unsafe {
            GetClientRect(window, &mut client)?;
            ClientToScreen(window, &mut origin).ok()?;
        }
        let observed_extent = [
            u32::try_from(i64::from(client.right) - i64::from(client.left))?,
            u32::try_from(i64::from(client.bottom) - i64::from(client.top))?,
        ];
        // SAFETY: no nearest-monitor fallback; selected output must still exist and contain
        // the complete client rectangle. A largest-intersection name alone is insufficient.
        let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONULL) };
        if monitor.0.is_null() {
            return Err("native output fixture does not intersect a monitor".into());
        }
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: MONITORINFOEXW begins with MONITORINFO and advertises its full writable size.
        unsafe { GetMonitorInfoW(monitor, std::ptr::from_mut(&mut info).cast()) }.ok()?;
        let end = info
            .szDevice
            .iter()
            .position(|value| *value == 0)
            .ok_or("unterminated monitor name")?;
        let observed_name = String::from_utf16(&info.szDevice[..end])?;
        let rect = info.monitorInfo.rcMonitor;
        let observed_rect = [rect.left, rect.top, rect.right, rect.bottom];
        if observed_name != self.device_name
            || observed_rect != self.desktop_rect
            || [origin.x, origin.y] != [self.desktop_rect[0], self.desktop_rect[1]]
            || observed_extent != self.extent
            || !fits(observed_rect, observed_extent)
        {
            return Err(format!(
                "native output fixture geometry mismatch: expected {} {:?} origin {:?} extent {:?}; observed {} {:?} origin {:?} extent {:?}",
                self.device_name, self.desktop_rect, &self.desktop_rect[..2], self.extent,
                observed_name, observed_rect, [origin.x, origin.y], observed_extent
            ).into());
        }
        Ok(())
    }
}

fn choose(outputs: &[OutputInfo], extent: [u32; 2]) -> Result<&OutputInfo, String> {
    if extent.contains(&0) {
        return Err("physical output extent must be nonzero".into());
    }
    let fitting: Vec<_> = outputs
        .iter()
        .filter(|output| output.attached && fits(output.desktop_rect, extent))
        .collect();
    let exact: Vec<_> = fitting
        .iter()
        .copied()
        .filter(|output| dimensions(output.desktop_rect) == extent.map(i64::from))
        .collect();
    let candidates = if exact.is_empty() { &fitting } else { &exact };
    match candidates.as_slice() {
        [output] => Ok(output),
        [] => Err(format!(
            "no attached display fits {}×{} physical pixels; select an adapter owning a fitting display",
            extent[0], extent[1]
        )),
        _ => Err(format!(
            "ambiguous fitting displays: {}; fixed-scene output requires one exact-size display or one fitting display",
            candidates
                .iter()
                .map(|output| output.device_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn dimensions([left, top, right, bottom]: [i32; 4]) -> [i64; 2] {
    [i64::from(right) - i64::from(left), i64::from(bottom) - i64::from(top)]
}

fn fits(rect: [i32; 4], extent: [u32; 2]) -> bool {
    let [width, height] = dimensions(rect);
    width >= i64::from(extent[0]) && height >= i64::from(extent[1])
}

#[cfg(test)]
mod tests;
