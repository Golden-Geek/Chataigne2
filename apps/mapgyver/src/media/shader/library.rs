//! Desktop catalog hosts resolve selections to self-contained source for the Media edit.
use reqwest::{Client, Response};
use std::time::Duration;
use tauri::{Manager, Runtime, async_runtime::Mutex, plugin::TauriPlugin};

mod fragcoord;
mod isf;

pub(crate) fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("mapgyver-shader-library")
        .setup(|app, _| {
            app.manage(Mutex::new(None::<isf::Catalog>));
            app.manage(Mutex::new(None::<fragcoord::Client>));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            isf::browse_isf,
            isf::load_isf,
            fragcoord::browse_fragcoord,
            fragcoord::load_fragcoord
        ])
        .build()
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("MapGyver Shader Library")
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|error| format!("Could not initialize the Shader Library: {error}"))
}

async fn bounded_body(mut response: Response, maximum: usize) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Could not read the library: {error}"))?
    {
        if chunk.len() > maximum - body.len() {
            return Err("The library response is too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
