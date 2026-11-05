use std::{cell::Cell, num::NonZeroIsize};

use raw_window_handle::{
  HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
};

use super::*;

struct Parent(HWND);

impl HasWindowHandle for Parent {
  fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
    let hwnd = self.0;
    let handle = Win32WindowHandle::new(NonZeroIsize::new(hwnd.0 as isize).unwrap());
    // SAFETY: this guard retains the same thread-owned HWND through the borrowed handle.
    Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)) })
  }
}

impl Drop for Parent {
  fn drop(&mut self) {
    // SAFETY: created and destroyed on this test's thread, after the WebView is dropped.
    let _ = unsafe { DestroyWindow(self.0) };
  }
}

const OBSERVER_ID: usize = 0x4757;

unsafe extern "system" fn reenter_after_detach(
  hwnd: HWND,
  msg: u32,
  wparam: WPARAM,
  lparam: LPARAM,
  _id: usize,
  data: usize,
) -> LRESULT {
  if msg == PARENT_DESTROY_MESSAGE {
    // SAFETY: the same-thread Box remains alive until this subclass is removed.
    let calls = unsafe { &*(data as *const Cell<u32>) };
    calls.set(calls.get() + 1);
    // These messages re-enter the chain while the outer detach is still on its stack.
    // They exercise the formerly cleared controller pointer without global input/focus.
    unsafe {
      SendMessageW(hwnd, WM_SETFOCUS, None, None);
      SendMessageW(hwnd, WM_MOVE, None, None);
      SendMessageW(hwnd, WM_SIZE, None, None);
    }
  }
  // SAFETY: continue this live same-thread subclass chain.
  unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

#[test]
#[ignore = "requires Windows and an installed WebView2 runtime"]
fn hardware_parent_subclass_is_removed_before_reentrant_focus_and_repeated_detach() {
  let hwnd;
  {
    // STATIC is a system window class; no persistent class registration is introduced.
    let parent = Parent(unsafe {
      CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Golden Wry teardown regression"),
        WS_OVERLAPPEDWINDOW,
        0,
        0,
        320,
        240,
        None,
        None,
        None,
        None,
      )
      .unwrap()
    });
    hwnd = parent.0;
    let calls = Box::new(Cell::new(0_u32));
    // Install below Wry so its detach forwards into this observer after releasing data.
    assert!(unsafe {
      SetWindowSubclass(
        hwnd,
        Some(reenter_after_detach),
        OBSERVER_ID,
        (&*calls as *const Cell<u32>) as usize,
      )
      .as_bool()
    });
    let profile = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../target")
      .join(format!("wry-parent-subclass-{}", std::process::id()));
    std::fs::create_dir_all(&profile).unwrap();
    let mut context = crate::WebContext::new(Some(profile));
    let view = crate::WebViewBuilder::new_with_web_context(&mut context)
      .with_visible(false)
      .with_focused(false)
      .build(&parent)
      .unwrap();
    let mut data = 0;
    assert!(unsafe {
      GetWindowSubclass(
        hwnd,
        Some(InnerWebView::parent_subclass_proc),
        PARENT_SUBCLASS_ID as _,
        Some(&mut data),
      )
      .as_bool()
    });
    assert_ne!(data, 0);

    unsafe { SendMessageW(hwnd, PARENT_DESTROY_MESSAGE, None, None) };
    assert_eq!(
      calls.get(),
      1,
      "the lower subclass must actually re-enter focus handling"
    );
    assert!(!unsafe {
      GetWindowSubclass(
        hwnd,
        Some(InnerWebView::parent_subclass_proc),
        PARENT_SUBCLASS_ID as _,
        Some(&mut data),
      )
      .as_bool()
    });
    unsafe { SendMessageW(hwnd, PARENT_DESTROY_MESSAGE, None, None) };
    assert_eq!(calls.get(), 2);
    drop(view);
    assert_eq!(
      calls.get(),
      3,
      "ordinary WebView drop may detach again safely"
    );
    assert!(unsafe {
      RemoveWindowSubclass(hwnd, Some(reenter_after_detach), OBSERVER_ID).as_bool()
    });
  }
  assert!(
    !unsafe { IsWindow(Some(hwnd)).as_bool() },
    "the parent HWND must retire normally"
  );
}
