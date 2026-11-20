//! Authenticated public catalog from the Shadertoy.com API.
use crate::media::protocol::{LibraryShader, ShaderLibraryEntry, ShaderLibraryPage};
use golden_video::shader::{FragmentEffect, FragmentFormat};
use reqwest::{StatusCode, Url, header};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashSet, num::NonZeroU16, sync::Arc};
use tauri::{State, async_runtime::Mutex};

const API: &str = "https://www.shadertoy.com/api/v1/shaders";
const MAX_REPLY: usize = 1024 * 1024;
const PAGE_SIZE: u32 = 8;
const UNAVAILABLE: &str = "This shader is unavailable through the ShaderToy API";

pub(super) struct Service {
    state: Mutex<Session>,
    fixture: Option<NonZeroU16>,
}

struct Session {
    key: Option<Arc<str>>,
    api: Option<Api>,
}

#[derive(Clone)]
struct Api {
    client: reqwest::Client,
    url: Url,
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(rename = "Shaders")]
    shaders: Vec<String>,
}

#[derive(Deserialize)]
struct ShaderResponse {
    #[serde(rename = "Shader")]
    shader: Value,
}

#[derive(Deserialize)]
struct ShaderInfo {
    id: String,
    name: String,
    username: String,
}

impl Service {
    pub(super) fn new() -> Self {
        #[cfg(debug_assertions)]
        let fixture = fixture_port();
        #[cfg(not(debug_assertions))]
        let fixture = None;
        let key = if fixture.is_none() {
            std::env::var("MAPGYVER_SHADERTOY_API_KEY")
                .ok()
                .and_then(|key| parse_key(&key).ok().flatten())
        } else {
            None
        };
        Self {
            state: Mutex::new(Session { key, api: None }),
            fixture,
        }
    }

    async fn credentials(&self) -> Result<(Api, Arc<str>), String> {
        let mut state = self.state.lock().await;
        let key = state.key.clone().ok_or("Enter a ShaderToy app key to browse")?;
        if let Some(api) = &state.api {
            return Ok((api.clone(), key));
        }
        let api = if let Some(port) = self.fixture {
            Api {
                client: super::http_client_builder()
                    .no_proxy()
                    .build()
                    .map_err(|_| "Could not open the fixture client")?,
                url: Url::parse(&format!("http://127.0.0.1:{port}/api/v1/shaders")).unwrap(),
            }
        } else {
            Api {
                client: super::http_client()?,
                url: Url::parse(API).unwrap(),
            }
        };
        state.api = Some(api.clone());
        Ok((api, key))
    }
}

#[cfg(debug_assertions)]
fn fixture_port() -> Option<NonZeroU16> {
    // Only an owned desktop automation launch can replace the fixed HTTPS origin.
    if !std::env::args_os().any(|arg| arg == "--automation-webview-debug-port") {
        return None;
    }
    std::env::var("MAPGYVER_AUTOMATION_SHADERTOY_PORT").ok()?.parse().ok()
}

#[tauri::command]
pub(super) async fn shadertoy_key_configured(service: State<'_, Service>) -> Result<bool, String> {
    Ok(service.state.lock().await.key.is_some())
}

#[tauri::command]
pub(super) async fn set_shadertoy_key(service: State<'_, Service>, key: String) -> Result<(), String> {
    let key = parse_key(&key)?;
    service.state.lock().await.key = key;
    Ok(())
}

fn parse_key(key: &str) -> Result<Option<Arc<str>>, String> {
    let key = key.trim();
    if key.is_empty() {
        return Ok(None);
    }
    if key.len() > 128
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("Invalid ShaderToy app key format".into());
    }
    Ok(Some(Arc::from(key)))
}

#[tauri::command]
pub(super) async fn browse_shadertoy(
    service: State<'_, Service>,
    query: String,
    offset: u32,
) -> Result<ShaderLibraryPage, String> {
    let query = query.trim();
    if query.len() > 400 || offset.checked_add(PAGE_SIZE).is_none() {
        return Err("Invalid ShaderToy search".into());
    }
    let (api, key) = service.credentials().await?;
    let mut segments = vec!["query"];
    if !query.is_empty() {
        segments.push(query);
    }
    let found: SearchResponse = api
        .request(
            &key,
            &segments,
            &[
                ("from", offset.to_string()),
                ("num", PAGE_SIZE.to_string()),
                ("sort", "popular".into()),
            ],
        )
        .await?;
    let has_next = found.shaders.len() >= PAGE_SIZE as usize;
    let mut seen = HashSet::new();
    let pending: Vec<_> = found
        .shaders
        .into_iter()
        .take(PAGE_SIZE as usize)
        .filter(|id| valid_id(id) && seen.insert(id.clone()))
        .map(|id| {
            let (api, key) = (api.clone(), key.clone());
            tauri::async_runtime::spawn(async move {
                let info = api.shader(&key, &id).await?.1;
                Ok::<_, String>(ShaderLibraryEntry { id, label: info.name })
            })
        })
        .collect();
    let mut entries = Vec::with_capacity(pending.len());
    for item in pending {
        match item.await.map_err(|_| "ShaderToy catalog request stopped")? {
            Ok(entry) => entries.push(entry),
            Err(error) if error == UNAVAILABLE => {}
            Err(error) => return Err(error),
        }
    }
    Ok(ShaderLibraryPage {
        entries,
        next_offset: has_next.then_some(offset + PAGE_SIZE),
    })
}

#[tauri::command]
pub(super) async fn load_shadertoy(service: State<'_, Service>, id: String) -> Result<LibraryShader, String> {
    if !valid_id(&id) {
        return Err("Invalid ShaderToy shader ID".into());
    }
    let (api, key) = service.credentials().await?;
    let (mut shader, info) = api.shader(&key, &id).await?;
    shader
        .as_object_mut()
        .ok_or("ShaderToy returned invalid source")?
        .insert(
            "mapgyver_source".into(),
            json!({
                "provider": "Shadertoy.com API",
                "url": format!("https://www.shadertoy.com/view/{}", info.id),
                "author": info.username,
                "terms": "https://www.shadertoy.com/terms"
            }),
        );
    let code = serde_json::to_string_pretty(&shader).map_err(|_| "Could not preserve the ShaderToy source")?;
    super::super::validate_code(&code)?;
    FragmentEffect::inspect_uniforms(FragmentFormat::ShaderToy, &code)
        .map_err(|error| format!("ShaderToy source cannot be imported: {error}"))?;
    Ok(LibraryShader {
        label: info.name,
        format: FragmentFormat::ShaderToy,
        code: Arc::from(code),
    })
}

impl Api {
    async fn shader(&self, key: &str, id: &str) -> Result<(Value, ShaderInfo), String> {
        let shader = self.request::<ShaderResponse>(key, &[id], &[]).await?.shader;
        let info = ShaderInfo::deserialize(&shader["info"]).map_err(|_| "ShaderToy returned invalid metadata")?;
        if info.id != id || !valid_label(&info.name) || !valid_label(&info.username) {
            return Err("ShaderToy returned invalid metadata".into());
        }
        Ok((shader, info))
    }

    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
        path: &[&str],
        parameters: &[(&str, String)],
    ) -> Result<T, String> {
        let mut url = self.url.clone();
        url.path_segments_mut()
            .expect("HTTPS API URL supports path segments")
            .extend(path.iter().copied());
        url.query_pairs_mut().append_pair("key", key).extend_pairs(parameters);
        let response = self
            .client
            .get(url)
            .header(header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|error| format!("Could not reach ShaderToy: {}", error.without_url()))?;
        match response.status() {
            StatusCode::UNAUTHORIZED => return Err("ShaderToy rejected the app key".into()),
            StatusCode::FORBIDDEN => return Err("ShaderToy denied access (HTTP 403)".into()),
            StatusCode::NOT_FOUND => return Err(UNAVAILABLE.into()),
            status if !status.is_success() => return Err(format!("ShaderToy returned HTTP {}", status.as_u16())),
            _ => {}
        }
        let body = super::bounded_body(response, MAX_REPLY).await?;
        let payload: Value = serde_json::from_slice(&body).map_err(|_| "ShaderToy did not return API data")?;
        if let Some(error) = payload.get("Error").and_then(Value::as_str) {
            if error.to_ascii_lowercase().contains("key") {
                return Err("ShaderToy rejected the app key".into());
            }
            return Err(UNAVAILABLE.into());
        }
        serde_json::from_value(payload).map_err(|_| "Invalid ShaderToy API response".into())
    }
}

fn valid_id(id: &str) -> bool {
    id.len() == 6 && id.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn valid_label(label: &str) -> bool {
    !label.trim().is_empty() && label.len() <= 240 && !label.chars().any(char::is_control)
}
