use super::NativeOutputBounds;
use std::{
    io,
    sync::{Mutex, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::ClientToScreen,
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::{
            HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext},
            WindowsAndMessaging::*,
        },
    },
    core::{PCWSTR, w},
};

const QUERY: u32 = WM_APP + 1;
const CLOSE: u32 = WM_APP + 2;
type Reply = mpsc::SyncSender<io::Result<NativeOutputBounds>>;

pub(super) struct Window {
    hwnd: isize,
    thread_id: u32,
    queries: mpsc::SyncSender<Reply>,
    thread: Option<JoinHandle<io::Result<()>>>,
}

impl Window {
    pub fn new(title: &str, bounds: NativeOutputBounds) -> io::Result<Self> {
        let title = title.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let (ready, startup) = mpsc::sync_channel(1);
        let (queries, requests) = mpsc::sync_channel::<Reply>(4);
        let thread = thread::Builder::new()
            .name("golden-output-window".into())
            .spawn(move || {
                // SAFETY: this dedicated thread creates, pumps and destroys its own HWND only.
                unsafe {
                    if SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_invalid() {
                        let error = io::Error::last_os_error();
                        let _ = ready.send(Err(error));
                        return Ok(());
                    }
                    let created = create(&title, bounds);
                    let hwnd = match created {
                        Ok(hwnd) => hwnd,
                        Err(error) => {
                            let _ = ready.send(Err(error));
                            return Ok(());
                        }
                    };
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    if ready.send(Ok((hwnd.0 as isize, GetCurrentThreadId()))).is_err() {
                        DestroyWindow(hwnd).map_err(io::Error::other)?;
                        return Ok(());
                    }
                    let mut message = MSG::default();
                    let result = loop {
                        let received = GetMessageW(&mut message, None, 0, 0).0;
                        if received == -1 {
                            break Err(io::Error::last_os_error());
                        }
                        if received == 0 || (message.hwnd.is_invalid() && message.message == CLOSE) {
                            break Ok(());
                        }
                        if message.hwnd.is_invalid() && message.message == QUERY {
                            while let Ok(reply) = requests.try_recv() {
                                let _ = reply.send(client_bounds(hwnd));
                            }
                        } else {
                            let _ = TranslateMessage(&message);
                            DispatchMessageW(&message);
                        }
                    };
                    // Native destruction is possible only after the owner requests close. WM_CLOSE
                    // from a system/user action is ignored while the caller may hold GPU use.
                    DestroyWindow(hwnd).map_err(io::Error::other)?;
                    result
                }
            })?;
        match startup.recv() {
            Ok(Ok((hwnd, thread_id))) => Ok(Self {
                hwnd,
                thread_id,
                queries,
                thread: Some(thread),
            }),
            result => {
                let _ = thread.join();
                match result {
                    Ok(Err(error)) => Err(error),
                    _ => Err(io::Error::other("output window startup disconnected")),
                }
            }
        }
    }
    pub fn handle(&self) -> isize {
        self.hwnd
    }
    pub fn bounds(&self) -> io::Result<NativeOutputBounds> {
        let (reply, result) = mpsc::sync_channel(1);
        self.queries.try_send(reply).map_err(io::Error::other)?;
        // SAFETY: the owned thread ID is live until close/join; no borrowed pointers cross it.
        unsafe { PostThreadMessageW(self.thread_id, QUERY, WPARAM(0), LPARAM(0)) }.map_err(io::Error::other)?;
        result.recv_timeout(Duration::from_secs(2)).map_err(io::Error::other)?
    }
    pub fn close(&mut self) -> io::Result<()> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        // SAFETY: only this owner posts the shutdown message, after its consumer's retirement.
        let posted =
            unsafe { PostThreadMessageW(self.thread_id, CLOSE, WPARAM(0), LPARAM(0)) }.map_err(io::Error::other);
        let joined = thread
            .join()
            .map_err(|_| io::Error::other("output window thread panicked"))?;
        posted.and(joined)
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("[golden-output-window] close failed: {error}");
        }
    }
}

unsafe fn create(title: &[u16], bounds: NativeOutputBounds) -> io::Result<HWND> {
    static CLASS: Mutex<bool> = Mutex::new(false);
    // SAFETY: module/class are process-owned; the fixed callback and name outlive all HWNDs.
    unsafe {
        let module = HINSTANCE(GetModuleHandleW(None).map_err(io::Error::other)?.0);
        // Serialize first registration and cache only success, so a transient native failure
        // remains recoverable on the next host attempt.
        let mut registered = CLASS.lock().unwrap_or_else(|error| error.into_inner());
        if !*registered {
            let atom = RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: module,
                lpszClassName: w!("GoldenNativeOutputWindow"),
                ..Default::default()
            });
            if atom == 0 {
                return Err(io::Error::last_os_error());
            }
            *registered = true;
        }
        drop(registered);
        CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            w!("GoldenNativeOutputWindow"),
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            bounds.left(),
            bounds.top(),
            bounds.width() as i32,
            bounds.height() as i32,
            None,
            None,
            Some(module),
            None,
        )
        .map_err(io::Error::other)
    }
}

unsafe fn client_bounds(hwnd: HWND) -> io::Result<NativeOutputBounds> {
    let mut rect = RECT::default();
    let mut origin = POINT::default();
    // SAFETY: this thread owns the live HWND and both output structs.
    unsafe {
        GetClientRect(hwnd, &mut rect).map_err(io::Error::other)?;
        if !ClientToScreen(hwnd, &mut origin).as_bool() {
            return Err(io::Error::last_os_error());
        }
    }
    NativeOutputBounds::new(
        origin.x,
        origin.y,
        (rect.right - rect.left) as u32,
        (rect.bottom - rect.top) as u32,
    )
}

unsafe extern "system" fn procedure(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_CLOSE | WM_DPICHANGED => LRESULT(0), // Physical bounds/lifetime belong to the owner.
        WM_ERASEBKGND => LRESULT(1),            // The GPU owns client pixels.
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}
