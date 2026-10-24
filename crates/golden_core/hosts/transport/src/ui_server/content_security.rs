use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dom_query::Document;
use sha2::{Digest, Sha256};

use super::UiAsset;

const MAX_HTML_ASSETS: usize = 256;
const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;
const MAX_INLINE_SCRIPTS: usize = 64;

/// Prepares immutable HTML policy off the engine/request paths. No HTML rewriting is required.
#[derive(Debug, Default)]
pub(super) struct FrontendSecurity {
    scripts: BTreeMap<&'static str, String>,
}

impl FrontendSecurity {
    pub(super) fn prepare(assets: &'static [UiAsset]) -> io::Result<Self> {
        let mut scripts = BTreeMap::new();
        for asset in assets.iter().filter(|asset| is_html(asset.content_type)) {
            if scripts.len() == MAX_HTML_ASSETS || asset.bytes.len() > MAX_HTML_BYTES {
                return Err(invalid("bundled HTML exceeds its CSP preparation limits"));
            }
            let html = std::str::from_utf8(asset.bytes).map_err(|_| invalid("bundled HTML must be UTF-8"))?;
            let document = Document::from(html);
            let inline = document.select("script:not([src])");
            if inline.length() > MAX_INLINE_SCRIPTS {
                return Err(invalid("bundled HTML has too many inline scripts"));
            }
            let mut hashes = BTreeSet::new();
            for script in inline.iter() {
                // HTML5 parsing normalizes line endings exactly as the browser does before hashing.
                hashes.insert(format!(
                    "'sha256-{}'",
                    STANDARD.encode(Sha256::digest(script.text().as_bytes()))
                ));
            }
            if scripts
                .insert(asset.path, hashes.into_iter().collect::<Vec<_>>().join(" "))
                .is_some()
            {
                return Err(invalid("duplicate bundled HTML path"));
            }
        }
        Ok(Self { scripts })
    }

    pub(super) fn policy(&self, asset: &UiAsset, authority: &str) -> Option<String> {
        let hashes = self.scripts.get(asset.path)?;
        Some(format!(
            "default-src 'self'; script-src 'self' {hashes}; style-src 'self' 'unsafe-inline'; \
             img-src 'self' data: blob:; font-src 'self' data:; \
             connect-src 'self' ws://{authority} ipc: http://ipc.localhost; \
             worker-src 'self' blob:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"
        ))
    }
}

fn is_html(content_type: &str) -> bool {
    content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .eq_ignore_ascii_case("text/html")
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests;
