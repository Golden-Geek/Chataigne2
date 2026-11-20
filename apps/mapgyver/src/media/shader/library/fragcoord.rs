//! Public FragCoord gallery. The provider returns license and source only for a selected shader.
use crate::media::protocol::{LibraryShader, ShaderLibraryEntry, ShaderLibraryPage};
use golden_video::shader::FragmentFormat;
use serde::Deserialize;
use serde_json::json;
use std::{fmt::Write as _, sync::Arc};
use tauri::{State, async_runtime::Mutex};

mod transport;
pub(super) use transport::Client;

const PAGE_SIZE: u32 = 25;
const MAX_OFFSET: u32 = 1000;

#[derive(Deserialize)]
struct SearchResult {
    total: u32,
    shaders: Vec<Summary>,
}

#[derive(Deserialize)]
struct Summary {
    slug: String,
    title: String,
}

#[derive(Deserialize)]
struct Shader {
    slug: String,
    title: String,
    author: String,
    source_available: bool,
    license: Option<License>,
    license_note: Option<String>,
    #[serde(default)]
    passes: Vec<Pass>,
}

#[derive(Deserialize)]
struct License {
    id: String,
    spdx: Option<String>,
    url: Option<String>,
}

#[derive(Deserialize)]
struct Pass {
    #[serde(rename = "type")]
    kind: String,
    language: String,
    code: String,
}

#[tauri::command]
pub(super) async fn browse_fragcoord(
    connection: State<'_, Mutex<Option<Client>>>,
    query: String,
    offset: u32,
) -> Result<ShaderLibraryPage, String> {
    let query = query.trim();
    if query.len() > 400 || offset > MAX_OFFSET {
        return Err("Invalid FragCoord search".into());
    }
    let found: SearchResult = transport::call(
        &connection,
        "search_shaders",
        json!({ "query": query, "offset": offset, "limit": PAGE_SIZE }),
    )
    .await?;
    let next = offset + PAGE_SIZE;
    let has_next = !found.shaders.is_empty() && next < found.total && next <= MAX_OFFSET;
    let entries = found
        .shaders
        .into_iter()
        .take(PAGE_SIZE as usize)
        .filter(|row| valid_slug(&row.slug) && valid_label(&row.title))
        .map(|row| ShaderLibraryEntry {
            id: row.slug,
            label: row.title,
        })
        .collect();
    Ok(ShaderLibraryPage {
        entries,
        next_offset: has_next.then_some(next),
    })
}

#[tauri::command]
pub(super) async fn load_fragcoord(
    connection: State<'_, Mutex<Option<Client>>>,
    id: String,
) -> Result<LibraryShader, String> {
    if !valid_slug(&id) {
        return Err("Invalid FragCoord shader".into());
    }
    let shader: Shader =
        transport::call(&connection, "get_shader", json!({ "slug": id, "include_source": true })).await?;
    shader.into_library_shader(&id)
}

impl Shader {
    fn into_library_shader(self, requested: &str) -> Result<LibraryShader, String> {
        if self.slug != requested || !valid_label(&self.title) || !valid_label(&self.author) {
            return Err("Invalid shader metadata from FragCoord".into());
        }
        if !self.source_available {
            return Err("This FragCoord shader does not publish its source".into());
        }
        let license = self.license.filter(|license| license.id != "none");
        let (license_id, license_url) = license
            .as_ref()
            .and_then(|license| Some((license.spdx.as_deref()?, license.url.as_deref()?)))
            .filter(|(id, url)| valid_label(id) && valid_label(url) && url.starts_with("https://"))
            .ok_or("This FragCoord shader has no reuse license")?;

        let (mut common, mut main) = (None, None);
        for pass in self.passes {
            if pass.language != "glsl" {
                return Err("MapGyver can import GLSL sources from FragCoord".into());
            }
            let destination = match pass.kind.as_str() {
                "common" => &mut common,
                "main" => &mut main,
                _ => return Err("FragCoord auxiliary passes are not supported for import yet".into()),
            };
            if destination.replace(pass.code).is_some() {
                return Err("Unexpected duplicate FragCoord pass".into());
            }
        }
        let main = main
            .filter(|code| !code.trim().is_empty())
            .ok_or("The FragCoord shader has no main source")?;
        let common = common.unwrap_or_default();
        let mut code = format!("{common}\n{main}\n\n");
        writeln!(
            code,
            "// Source: https://fragcoord.xyz/s/{} by {}",
            self.slug, self.author
        )
        .unwrap();
        writeln!(code, "// License: {license_id} ({license_url})").unwrap();
        if let Some(note) = self.license_note {
            for line in note.lines() {
                writeln!(code, "// {line}").unwrap();
            }
        }
        super::super::validate_code(&code)?;
        Ok(LibraryShader {
            label: self.title,
            format: FragmentFormat::FragCoord,
            code: Arc::from(code),
        })
    }
}

fn valid_label(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 240 && !value.chars().any(char::is_control)
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}
