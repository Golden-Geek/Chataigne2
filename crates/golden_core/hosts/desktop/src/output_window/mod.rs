//! Independent native output windows. GPU consumers borrow their handles and must retire before
//! closing the owner. No WebView, engine work or periodic polling belongs on these window threads.

use std::io;
#[cfg(windows)]
mod platform;

/// Checked client bounds in signed physical desktop pixels, independent of CSS/OS logical units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeOutputBounds {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
}

impl NativeOutputBounds {
    /// Reject empty/unrepresentable client extents and overflowing desktop edges.
    pub fn new(left: i32, top: i32, width: u32, height: u32) -> io::Result<Self> {
        if width == 0
            || height == 0
            || width > i32::MAX as u32
            || height > i32::MAX as u32
            || left.checked_add_unsigned(width).is_none()
            || top.checked_add_unsigned(height).is_none()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid physical output rectangle",
            ));
        }
        Ok(Self {
            left,
            top,
            width,
            height,
        })
    }
    /// Physical desktop x coordinate; negative monitor origins are preserved.
    pub fn left(self) -> i32 {
        self.left
    }
    /// Physical desktop y coordinate.
    pub fn top(self) -> i32 {
        self.top
    }
    /// Physical client width.
    pub fn width(self) -> u32 {
        self.width
    }
    /// Physical client height.
    pub fn height(self) -> u32 {
        self.height
    }
}

/// Keyboard input from a focused native output window. The host does not assign app semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeOutputEvent {
    /// A fresh Escape key press; auto-repeat and unfocused input are ignored.
    EscapePressed,
}

/// Host-owned, borderless, topmost output window with no initial focus activation or user resize/close.
/// Each owner has an event-driven window thread with per-monitor-v2 physical-coordinate semantics.
/// Construct/query/close off the application's UI/engine thread. The HWND stays alive until close
/// or Drop; callers borrowing it for a renderer must finish actual GPU retirement first.
#[non_exhaustive]
pub struct NativeOutputWindow {
    #[cfg(windows)]
    inner: platform::Window,
}

impl NativeOutputWindow {
    /// Create a dedicated output window. Title is bounded metadata, never a persisted handle.
    pub fn new(title: &str, bounds: NativeOutputBounds) -> io::Result<Self> {
        Self::with_events(title, bounds, |_| {})
    }

    /// Create without stealing focus; clicking the output can focus it for keyboard input.
    /// The callback runs on its dedicated window thread and must only signal/queue bounded work.
    /// It must not perform engine, rendering, blocking IO or window teardown work.
    pub fn with_events(
        title: &str,
        bounds: NativeOutputBounds,
        on_event: impl Fn(NativeOutputEvent) + Send + 'static,
    ) -> io::Result<Self> {
        if title.len() > 512 || title.contains('\0') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid output window title",
            ));
        }
        #[cfg(windows)]
        {
            Ok(Self {
                inner: platform::Window::new(title, bounds, Box::new(on_event))?,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (bounds, on_event);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "native output windows require Windows",
            ))
        }
    }
    /// Borrow the HWND without extending ownership. Keep this owner alive through GPU retirement.
    pub fn raw_handle(&self) -> isize {
        #[cfg(windows)]
        {
            self.inner.handle()
        }
        #[cfg(not(windows))]
        {
            unreachable!("unsupported platform cannot construct an output window")
        }
    }
    /// Query actual client geometry on the window's DPI-aware thread; this is metadata only.
    pub fn physical_bounds(&self) -> io::Result<NativeOutputBounds> {
        #[cfg(windows)]
        {
            self.inner.bounds()
        }
        #[cfg(not(windows))]
        {
            unreachable!("unsupported platform cannot construct an output window")
        }
    }
    /// Close/join after the caller's GPU consumers retire. Returns only after native destruction.
    pub fn close(self) -> io::Result<()> {
        #[cfg(windows)]
        {
            let mut inner = self.inner;
            inner.close()
        }
        #[cfg(not(windows))]
        {
            unreachable!("unsupported platform cannot construct an output window")
        }
    }
}

#[cfg(test)]
mod tests;
