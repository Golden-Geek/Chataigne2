//! Read native paths from WebView2's DOM File objects while preserving HTML drag-and-drop.
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{Runtime, Url, WebviewWindow};
use webview2_com::{
    Microsoft::Web::WebView2::Win32::{ICoreWebView2, ICoreWebView2File, ICoreWebView2WebMessageReceivedEventArgs2},
    WebMessageReceivedEventHandler, take_pwstr,
};
use windows::core::{Interface, PCWSTR, PWSTR};

const MESSAGE: &str = "golden:file-drop";
const MAX_FILES: u32 = 64;

#[derive(Deserialize)]
struct Request {
    kind: String,
    id: String,
}

#[derive(Serialize)]
struct Reply<'a> {
    kind: &'static str,
    id: &'a str,
    paths: Vec<String>,
}

pub(super) fn attach<R: Runtime>(window: &WebviewWindow<R>, frontend: Url) -> tauri::Result<()> {
    window.with_webview(move |webview| {
        // SAFETY: with_webview invokes this closure on the WebView2 UI thread.
        let result = unsafe {
            webview
                .controller()
                .CoreWebView2()
                .and_then(|webview| register(&webview, frontend))
        };
        if let Err(error) = result {
            eprintln!("warning: could not register WebView2 file drops: {error}");
        }
    })
}

fn register(webview: &ICoreWebView2, frontend: Url) -> windows::core::Result<()> {
    let origin = frontend.origin();
    let handler = WebMessageReceivedEventHandler::create(Box::new(move |webview, args| {
        let (Some(webview), Some(args)) = (webview, args) else {
            return Ok(());
        };
        let mut value = PWSTR::null();
        // SAFETY: callback and its COM objects belong to this WebView2 UI thread. Returned
        // strings transfer ownership to take_pwstr; additional objects are scoped to the event.
        unsafe {
            args.WebMessageAsJson(&mut value)?;
            let Ok(request) = serde_json::from_str::<Request>(&take_pwstr(value)) else {
                return Ok(());
            };
            if request.kind != MESSAGE || request.id.len() != 36 {
                return Ok(());
            }
            args.Source(&mut value)?;
            if !Url::parse(&take_pwstr(value)).is_ok_and(|url| url.origin() == origin) {
                return Ok(());
            }
            let paths = args
                .cast::<ICoreWebView2WebMessageReceivedEventArgs2>()
                .ok()
                .and_then(|args| read_paths(&args))
                .unwrap_or_default();
            webview.Source(&mut value)?;
            if !Url::parse(&take_pwstr(value)).is_ok_and(|url| url.origin() == origin) {
                return Ok(());
            }
            let reply = serde_json::to_string(&Reply {
                kind: MESSAGE,
                id: &request.id,
                paths,
            })
            .expect("file-drop reply contains JSON strings");
            let message: Vec<u16> = reply.encode_utf16().chain([0]).collect();
            webview.PostWebMessageAsJson(PCWSTR(message.as_ptr()))?;
        }
        Ok(())
    }));
    // SAFETY: register is called on the same WebView2 UI thread. The webview owns the handler
    // until it is destroyed, and the callback uses the sender passed with the event.
    let mut registration = 0;
    unsafe { webview.add_WebMessageReceived(&handler, &mut registration) }
}

fn read_paths(args: &ICoreWebView2WebMessageReceivedEventArgs2) -> Option<Vec<String>> {
    // SAFETY: these WebView2 file objects are accessed on their owning event thread.
    unsafe {
        let objects = args.AdditionalObjects().ok()?;
        let mut count = 0;
        objects.Count(&mut count).ok()?;
        if !(1..=MAX_FILES).contains(&count) {
            return None;
        }
        (0..count)
            .map(|index| {
                let file = objects.GetValueAtIndex(index).ok()?.cast::<ICoreWebView2File>().ok()?;
                let mut value = PWSTR::null();
                file.Path(&mut value).ok()?;
                let path = take_pwstr(value);
                Path::new(&path).is_absolute().then_some(path)
            })
            .collect()
    }
}
