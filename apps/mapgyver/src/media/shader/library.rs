//! The desktop library resolves catalog selections to bounded, self-contained shader source.
use crate::media::protocol::{LibraryShader, ShaderLibraryEntry};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use golden_video::shader::{FragmentFormat, MAX_SOURCE_BYTES};
use reqwest::Client;
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tauri::{Manager, Runtime, State, async_runtime::Mutex, plugin::TauriPlugin};

const UPSTREAM: &str = "https://github.com/Vidvox/ISF-Files";
const API: &str = "https://api.github.com/repos/Vidvox/ISF-Files/git/";
const MAX_CATALOG_BYTES: usize = 1024 * 1024;
const MAX_LICENSE_BYTES: usize = 8 * 1024;

pub(crate) fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("mapgyver-shader-library")
        .setup(|app, _| {
            app.manage(Mutex::new(None::<Catalog>));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![browse_isf, load_isf])
        .build()
}

struct Catalog {
    client: Client,
    entries: Vec<Entry>,
    license: Arc<str>,
}

#[derive(Clone)]
struct Entry {
    public: ShaderLibraryEntry,
    path: String,
    sha: String,
}

#[derive(Deserialize)]
struct GitTree {
    tree: Vec<GitEntry>,
    truncated: bool,
}

#[derive(Deserialize)]
struct GitEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
    sha: String,
    size: Option<usize>,
}

#[derive(Deserialize)]
struct GitBlob {
    encoding: String,
    content: String,
}

#[tauri::command]
async fn browse_isf(
    catalog: State<'_, Mutex<Option<Catalog>>>,
    refresh: bool,
) -> Result<Vec<ShaderLibraryEntry>, String> {
    let mut catalog = catalog.lock().await;
    if catalog.is_none() || refresh {
        let client = match catalog.as_ref() {
            Some(previous) => previous.client.clone(),
            None => Client::builder()
                .https_only(true)
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("MapGyver Shader Library")
                .timeout(Duration::from_secs(12))
                .build()
                .map_err(|error| format!("Could not initialize the ISF library: {error}"))?,
        };
        *catalog = Some(fetch_catalog(client).await?);
    }
    Ok(catalog
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .map(|entry| entry.public.clone())
        .collect())
}

#[tauri::command]
async fn load_isf(catalog: State<'_, Mutex<Option<Catalog>>>, id: String) -> Result<LibraryShader, String> {
    let (client, entry, license) = {
        let catalog = catalog.lock().await;
        let catalog = catalog.as_ref().ok_or("Open the ISF library first")?;
        let entry = catalog
            .entries
            .iter()
            .find(|entry| entry.public.id == id)
            .ok_or("This shader is no longer in the ISF catalog")?;
        (catalog.client.clone(), entry.clone(), catalog.license.clone())
    };
    let mut code = fetch_blob(&client, &entry.sha, MAX_SOURCE_BYTES).await?;
    if code.trim().is_empty() {
        return Err("The ISF shader is empty".into());
    }
    // Preserve the original leading ISF metadata and carry the license into saved projects.
    use std::fmt::Write as _;
    writeln!(code, "\n\n// Source: {UPSTREAM}, {} (blob {})", entry.path, entry.sha).unwrap();
    for line in license.lines() {
        writeln!(code, "// {line}").unwrap();
    }
    super::validate_code(&code)?;
    Ok(LibraryShader {
        label: entry.public.label,
        format: FragmentFormat::Isf,
        code: Arc::from(code),
    })
}

async fn fetch_catalog(client: Client) -> Result<Catalog, String> {
    let tree: GitTree = request(&client, "trees/master?recursive=1", MAX_CATALOG_BYTES).await?;
    if tree.truncated {
        return Err("The ISF catalog is incomplete".into());
    }
    let license = tree
        .tree
        .iter()
        .find(|item| item.path == "LICENSE" && item.is_blob_within(MAX_LICENSE_BYTES))
        .ok_or("The ISF catalog license is missing")?;
    let license = fetch_blob(&client, &license.sha, MAX_LICENSE_BYTES).await?;
    if license.trim().is_empty() {
        return Err("The ISF catalog license is empty".into());
    }
    let mut entries: Vec<Entry> = tree
        .tree
        .into_iter()
        .filter_map(|item| {
            let file = item.path.strip_prefix("ISF/")?;
            let (label, FragmentFormat::Isf) = super::file_format(file)? else {
                return None;
            };
            if file.contains('/')
                || label.trim().is_empty()
                || label.len() > 240
                || label.chars().any(char::is_control)
                || !item.is_blob_within(MAX_SOURCE_BYTES - license.len() - 1024)
            {
                return None;
            }
            Some(Entry {
                public: ShaderLibraryEntry {
                    id: format!("{}:{}", item.sha, item.path),
                    label: label.to_owned(),
                },
                path: item.path,
                sha: item.sha,
            })
        })
        .collect();
    entries.sort_by_cached_key(|entry| (entry.public.label.to_lowercase(), entry.path.clone()));
    if entries.is_empty() {
        return Err("The ISF catalog contains no shaders".into());
    }
    Ok(Catalog {
        client,
        entries,
        license: Arc::from(license),
    })
}

impl GitEntry {
    fn is_blob_within(&self, maximum: usize) -> bool {
        self.kind == "blob"
            && self.size.is_some_and(|size| size > 0 && size <= maximum)
            && self.sha.len() == 40
            && self.sha.bytes().all(|byte| byte.is_ascii_hexdigit())
    }
}

async fn fetch_blob(client: &Client, sha: &str, maximum: usize) -> Result<String, String> {
    let blob: GitBlob = request(client, &format!("blobs/{sha}"), 2 * maximum + 2048).await?;
    if blob.encoding != "base64" {
        return Err("Unexpected encoding in the ISF library response".into());
    }
    let mut content = blob.content.into_bytes();
    content.retain(|byte| !byte.is_ascii_whitespace());
    let bytes = STANDARD.decode(content).map_err(|_| "Invalid ISF library response")?;
    if bytes.len() > maximum {
        return Err("The downloaded ISF file is too large".into());
    }
    String::from_utf8(bytes).map_err(|_| "The downloaded ISF file is not UTF-8".into())
}

async fn request<T: serde::de::DeserializeOwned>(client: &Client, resource: &str, maximum: usize) -> Result<T, String> {
    let mut response = client
        .get(format!("{API}{resource}"))
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("Could not reach the ISF library: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("The ISF library returned HTTP {}", response.status().as_u16()));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Could not read the ISF library: {error}"))?
    {
        if chunk.len() > maximum - body.len() {
            return Err("The ISF library response is too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| "Invalid ISF library response".into())
}
