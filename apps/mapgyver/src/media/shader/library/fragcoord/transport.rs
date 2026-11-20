//! The fixed public MCP endpoint used by the FragCoord gallery.
use reqwest::{Response, StatusCode, header};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use tauri::async_runtime::Mutex;

const ENDPOINT: &str = "https://fragcoord.xyz/mcp";
const PROTOCOL: &str = "2025-03-26";
const MAX_REPLY: usize = 1024 * 1024;
const SESSION_HEADER: &str = "mcp-session-id";

pub(in super::super) struct Client {
    http: reqwest::Client,
    session: Option<header::HeaderValue>,
    next_id: u64,
}

enum RequestError {
    Expired,
    Failure(String),
}

impl From<String> for RequestError {
    fn from(error: String) -> Self {
        Self::Failure(error)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolResult {
    #[serde(default)]
    is_error: bool,
    structured_content: Option<Value>,
    #[serde(default)]
    content: Vec<Content>,
}

#[derive(Deserialize)]
struct Content {
    text: Option<String>,
}

pub(super) async fn call<T: DeserializeOwned>(
    connection: &Mutex<Option<Client>>,
    tool: &str,
    arguments: Value,
) -> Result<T, String> {
    let mut connection = connection.lock().await;
    for _ in 0..2 {
        if connection.is_none() {
            *connection = Some(Client::connect().await?);
        }
        let client = connection.as_mut().unwrap();
        let payload = match client.tool(tool, &arguments).await {
            Ok(result) => result,
            Err(RequestError::Expired) => {
                *connection = None;
                continue;
            }
            Err(RequestError::Failure(error)) => return Err(error),
        };
        return serde_json::from_value(payload).map_err(|_| "Unexpected response from FragCoord".into());
    }
    Err("The FragCoord session expired. Try again to reconnect".into())
}

impl Client {
    async fn connect() -> Result<Self, String> {
        let mut client = Self {
            http: super::super::http_client()?,
            session: None,
            next_id: 2,
        };
        let response = client
            .post(json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {
                    "protocolVersion": PROTOCOL,
                    "capabilities": {},
                    "clientInfo": { "name": "MapGyver", "version": env!("CARGO_PKG_VERSION") }
                }
            }))
            .await?;
        client.session = response.headers().get(SESSION_HEADER).cloned();
        if reply(response, 1).await?.get("protocolVersion").and_then(Value::as_str) != Some(PROTOCOL) {
            return Err("FragCoord returned an unsupported protocol version".into());
        }
        let initialized = client
            .post(json!({"jsonrpc":"2.0", "method":"notifications/initialized"}))
            .await?;
        if !initialized.status().is_success() {
            return Err(format!(
                "Could not initialize FragCoord (HTTP {})",
                initialized.status().as_u16()
            ));
        }
        Ok(client)
    }

    async fn tool(&mut self, tool: &str, arguments: &Value) -> Result<Value, RequestError> {
        let id = self.next_id;
        self.next_id += 1;
        let response = self
            .post(json!({
                "jsonrpc":"2.0", "id": id, "method":"tools/call",
                "params": { "name": tool, "arguments": arguments }
            }))
            .await?;
        if self.session.is_some() && response.status() == StatusCode::NOT_FOUND {
            return Err(RequestError::Expired);
        }
        let result: ToolResult = serde_json::from_value(reply(response, id).await?)
            .map_err(|_| "Invalid FragCoord tool response".to_owned())?;
        let text = result.content.iter().find_map(|item| item.text.as_deref());
        if result.is_error {
            return Err(format!("FragCoord: {}", short_message(text.unwrap_or("the request failed"))).into());
        }
        if let Some(payload) = result.structured_content {
            return Ok(payload);
        }
        serde_json::from_str(text.unwrap_or_default()).map_err(|_| "Invalid FragCoord data".to_owned().into())
    }

    async fn post(&self, body: Value) -> Result<Response, String> {
        let mut request = self
            .http
            .post(ENDPOINT)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream")
            .header("mcp-protocol-version", PROTOCOL)
            .body(body.to_string());
        if let Some(session) = &self.session {
            request = request.header(SESSION_HEADER, session);
        }
        request
            .send()
            .await
            .map_err(|error| format!("Could not reach FragCoord: {error}"))
    }
}

async fn reply(response: Response, id: u64) -> Result<Value, String> {
    if !response.status().is_success() {
        return Err(format!("FragCoord returned HTTP {}", response.status().as_u16()));
    }
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    let value = if content_type.is_some_and(|mime| mime.starts_with("text/event-stream")) {
        event_reply(response, id).await?
    } else {
        let body = super::super::bounded_body(response, MAX_REPLY).await?;
        serde_json::from_slice(&body).map_err(|_| "Invalid FragCoord response")?
    };
    if value.get("id").and_then(Value::as_u64) != Some(id) {
        return Err("FragCoord returned the wrong request ID".into());
    }
    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("the request failed");
        return Err(format!("FragCoord: {}", short_message(message)));
    }
    value
        .get("result")
        .cloned()
        .ok_or_else(|| "FragCoord returned no result".into())
}

async fn event_reply(mut response: Response, id: u64) -> Result<Value, String> {
    let (mut body, mut event) = (Vec::new(), Vec::new());
    let mut cursor = 0;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Could not read FragCoord: {error}"))?
    {
        if chunk.len() > MAX_REPLY - body.len() {
            return Err("The FragCoord response is too large".into());
        }
        body.extend_from_slice(&chunk);
        while let Some(end) = body[cursor..].iter().position(|&byte| byte == b'\n') {
            let line = &body[cursor..cursor + end];
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            cursor += end + 1;
            if let Some(data) = line.strip_prefix(b"data:") {
                event.extend_from_slice(data.strip_prefix(b" ").unwrap_or(data));
                event.push(b'\n');
            } else if line.is_empty() && !event.is_empty() {
                let reply = serde_json::from_slice::<Value>(&event).map_err(|_| "Invalid FragCoord event")?;
                if reply.get("id").and_then(Value::as_u64) == Some(id) {
                    return Ok(reply);
                }
                event.clear();
            }
        }
    }
    Err("FragCoord closed the response before returning the result".into())
}

fn short_message(message: &str) -> String {
    message.chars().take(240).collect()
}
