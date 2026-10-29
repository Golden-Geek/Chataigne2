use std::io::{self, Error, ErrorKind};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::window::Color;
use tauri::{Manager, Runtime, Url, WebviewUrl};

use crate::native_presentation::{
    NativePresentation,
    host::{ExitAction, PresentationHost},
};
use crate::window_state::{WindowStatePersistence, restore_window_state, save_window_state};

const WINDOW_CLOSE_REQUESTED_SCRIPT: &str = "window.dispatchEvent(new CustomEvent('gc-window-close-requested'));";

pub(super) fn run_tauri<R: Runtime>(
    ui_base_url: &str,
    application_display_name: String,
    tauri_context: tauri::Context<R>,
    window_state_path: PathBuf,
    automation_shutdown_file: Option<PathBuf>,
    native_presentation: Option<NativePresentation<R>>,
) -> io::Result<()> {
    let external_url: Url = ui_base_url.parse().map_err(|err| {
        Error::new(
            ErrorKind::InvalidInput,
            format!("invalid UI URL '{ui_base_url}': {err}"),
        )
    })?;

    #[cfg(target_os = "linux")]
    unsafe {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }

    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "macos"
    };
    // Document-start precedes <html>, so defer the platform attribute when necessary.
    let init_script = format!(
        "window.__PLATFORM__ = '{os}'; (function () {{ var apply = function () {{ document.documentElement.dataset.platform = '{os}'; }}; if (document.documentElement) {{ apply(); }} else {{ document.addEventListener('DOMContentLoaded', apply); }} }})();"
    );
    let window_state = WindowStatePersistence::new(window_state_path);
    let presentation_host = native_presentation
        .as_ref()
        .map(|_| Arc::new(PresentationHost::default()));
    let close_presentation = presentation_host.clone();
    let setup_presentation = presentation_host.clone();
    let mut builder = tauri::Builder::<R>::new().manage(window_state.clone());
    if let Some(host) = presentation_host.as_ref() {
        builder = builder.manage(host.clone());
    }

    let app = builder
        .on_window_event(move |window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if let Some(state) = window.try_state::<WindowStatePersistence>() {
                    save_window_state(window, &state);
                }
                if dispatch_close_requested(window) {
                    // The workbench still owns the user's save/discard/cancel decision.
                    api.prevent_close();
                } else if close_presentation.is_some() {
                    api.prevent_close();
                    window.app_handle().exit(0);
                }
            }
        })
        .setup(move |app| {
            let setup = create_main_window(
                app,
                external_url,
                application_display_name,
                &window_state,
                native_presentation.is_some(),
            )
            .and_then(|window| {
                if let Some(presentation) = native_presentation {
                    setup_presentation.as_ref().unwrap().attach(presentation, &window)?;
                }
                start_automation_shutdown(app.handle(), automation_shutdown_file);
                Ok(())
            });
            if let Err(error) = setup {
                if let Some(host) = setup_presentation {
                    // Tauri panics on setup Err. No worker starts before preparation succeeds;
                    // retain the error and leave through the ordinary exit/finish path instead.
                    host.fail_before_start(error);
                    app.handle().exit(1);
                } else {
                    return Err(error.into());
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::desktop_commands::native_presentation_control,
            crate::desktop_commands::window_minimize,
            crate::desktop_commands::window_toggle_maximize,
            crate::desktop_commands::window_close,
            crate::desktop_commands::window_destroy,
            crate::desktop_commands::window_is_maximized,
            crate::desktop_commands::start_drag,
            crate::desktop_commands::open_file_dialog,
            crate::desktop_commands::save_file_dialog,
            crate::desktop_commands::write_app_data_file,
            crate::desktop_commands::write_file_in_directory
        ])
        .append_invoke_initialization_script(&init_script)
        .build(tauri_context)
        .map_err(|err| Error::other(format!("tauri runtime failed: {err}")))?;

    if let Some(host) = presentation_host {
        let exit_host = host.clone();
        let code = app.run_return(move |app, event| match event {
            tauri::RunEvent::ExitRequested { code, api, .. } => match exit_host.exit_action(code.unwrap_or(0)) {
                ExitAction::Wait => api.prevent_exit(),
                ExitAction::DestroyWindow => {
                    api.prevent_exit();
                    if let Some(window) = app.get_webview_window("main") {
                        if let Err(error) = window.destroy() {
                            app.exit(exit_host.teardown_failed(Error::other(error)));
                        }
                    } else if let Some(code) = exit_host.window_destroyed() {
                        app.exit(code);
                    }
                }
                ExitAction::Redirect(code) => {
                    api.prevent_exit();
                    app.exit(code);
                }
                ExitAction::Allow => {}
            },
            tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::Destroyed,
                ..
            } if label == "main" => {
                if let Some(code) = exit_host.window_destroyed() {
                    app.exit(code);
                }
            }
            _ => {}
        });
        host.finish()?;
        if code != 0 {
            return Err(Error::other(format!(
                "native presentation host exited with status {code}"
            )));
        }
    } else {
        app.run(|_, _| {});
    }
    Ok(())
}

fn create_main_window<R: Runtime>(
    app: &tauri::App<R>,
    url: Url,
    title: String,
    window_state: &WindowStatePersistence,
    native_underlay: bool,
) -> io::Result<tauri::WebviewWindow<R>> {
    let mut builder = tauri::WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
        .title(title)
        .decorations(false)
        .shadow(true)
        .accept_first_mouse(true)
        .inner_size(75.0 * 16.0, 50.0 * 16.0);
    if cfg!(target_os = "windows") {
        builder = builder.disable_drag_drop_handler();
        builder = if native_underlay {
            builder.transparent(true).background_color(Color(0, 0, 0, 0))
        } else {
            builder.background_color(Color(20, 20, 20, 255))
        };
    }
    let window = builder
        .build()
        .map_err(|err| Error::other(format!("failed creating Tauri window: {err}")))?;
    restore_window_state(&window, window_state);
    Ok(window)
}

fn dispatch_close_requested<R: Runtime>(window: &tauri::Window<R>) -> bool {
    let mut delivered = false;
    for webview in window.webviews() {
        match webview.eval(WINDOW_CLOSE_REQUESTED_SCRIPT) {
            Ok(()) => delivered = true,
            Err(err) => eprintln!(
                "warning: failed to dispatch close-request event to frontend for window '{}': {err}",
                window.label()
            ),
        }
    }
    delivered
}

fn start_automation_shutdown<R: Runtime>(app: &tauri::AppHandle<R>, path: Option<PathBuf>) {
    let Some(path) = path else { return };
    let app = app.clone();
    thread::spawn(move || {
        loop {
            match std::fs::metadata(&path) {
                Ok(_) => {
                    eprintln!("automation shutdown requested by sentinel {}", path.display());
                    app.exit(0);
                    break;
                }
                Err(err) if err.kind() == ErrorKind::NotFound => thread::sleep(Duration::from_millis(100)),
                Err(err) => {
                    eprintln!(
                        "warning: automation shutdown sentinel {} cannot be observed: {err}",
                        path.display()
                    );
                    break;
                }
            }
        }
    });
}
