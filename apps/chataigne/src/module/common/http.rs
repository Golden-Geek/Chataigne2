use golden_core::parameter::{ParamValue, ParameterEnumOption};
use serde::{Deserialize, Serialize};

use crate::app::module::common::received_values::ReceivedValuePayload;

pub(crate) const HTTP_REQUEST_COMMAND_NODE_TYPE: &str = "http_request_command";
pub(crate) const HTTP_UPLOAD_FILE_COMMAND_NODE_TYPE: &str = "http_upload_file_command";
pub(crate) const HTTP_COMMAND_NODE_TYPES: &[&str] =
    &[HTTP_REQUEST_COMMAND_NODE_TYPE, HTTP_UPLOAD_FILE_COMMAND_NODE_TYPE];

pub(crate) const HTTP_METHOD_GET: &str = "get";
pub(crate) const HTTP_METHOD_POST: &str = "post";
pub(crate) const HTTP_METHOD_PUT: &str = "put";
pub(crate) const HTTP_METHOD_PATCH: &str = "patch";
pub(crate) const HTTP_METHOD_DELETE: &str = "delete";
pub(crate) const HTTP_METHOD_HEAD: &str = "head";
pub(crate) const HTTP_METHOD_OPTIONS: &str = "options";

pub(crate) const HTTP_AUTH_NONE: &str = "none";
pub(crate) const HTTP_AUTH_BASIC: &str = "basic";
pub(crate) const HTTP_AUTH_BEARER: &str = "bearer";
pub(crate) const HTTP_AUTH_HEADER: &str = "header";

pub(crate) const HTTP_BODY_NONE: &str = "none";
pub(crate) const HTTP_BODY_TEXT: &str = "text";
pub(crate) const HTTP_BODY_JSON: &str = "json";
pub(crate) const HTTP_BODY_FORM: &str = "form";

pub(crate) const HTTP_TEXT_CONTENT_TYPE: &str = "text/plain; charset=utf-8";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HttpMethod {
    #[default]
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    pub(crate) fn from_variant(variant: &str) -> Option<Self> {
        match normalized_variant(variant).as_str() {
            HTTP_METHOD_GET => Some(Self::Get),
            HTTP_METHOD_POST => Some(Self::Post),
            HTTP_METHOD_PUT => Some(Self::Put),
            HTTP_METHOD_PATCH => Some(Self::Patch),
            HTTP_METHOD_DELETE => Some(Self::Delete),
            HTTP_METHOD_HEAD => Some(Self::Head),
            HTTP_METHOD_OPTIONS => Some(Self::Options),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }

    pub(crate) fn to_reqwest(self) -> reqwest::Method {
        match self {
            Self::Get => reqwest::Method::GET,
            Self::Post => reqwest::Method::POST,
            Self::Put => reqwest::Method::PUT,
            Self::Patch => reqwest::Method::PATCH,
            Self::Delete => reqwest::Method::DELETE,
            Self::Head => reqwest::Method::HEAD,
            Self::Options => reqwest::Method::OPTIONS,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct HttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HttpAuthMode {
    #[default]
    None,
    Basic,
    Bearer,
    Header,
}

impl HttpAuthMode {
    pub(crate) fn from_variant(variant: &str) -> Option<Self> {
        match normalized_variant(variant).as_str() {
            HTTP_AUTH_NONE => Some(Self::None),
            HTTP_AUTH_BASIC => Some(Self::Basic),
            HTTP_AUTH_BEARER => Some(Self::Bearer),
            HTTP_AUTH_HEADER => Some(Self::Header),
            _ => None,
        }
    }

}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct HttpAuthConfig {
    pub mode: HttpAuthMode,
    pub username: String,
    pub password: String,
    pub token: String,
    pub header_name: String,
    pub header_value: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum HttpRequestBody {
    #[default]
    Empty,
    Text {
        text: String,
        content_type: String,
    },
    Json {
        value: serde_json::Value,
    },
    Form {
        fields: Vec<HttpFormField>,
    },
    Multipart {
        fields: Vec<HttpFormField>,
        files: Vec<HttpFilePart>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct HttpFormField {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct HttpFilePart {
    pub field_name: String,
    pub path: String,
    pub file_name: Option<String>,
    pub content_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct HttpRequestPayload {
    pub method: HttpMethod,
    pub path: String,
    pub query: String,
    pub value_path: String,
    pub headers: Vec<HttpHeader>,
    pub body: HttpRequestBody,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HttpReceivedValue {
    pub path_segments: Vec<String>,
    pub payload: ReceivedValuePayload,
    pub source_description: String,
}

pub(crate) fn http_method_enum_options() -> Vec<ParameterEnumOption> {
    enum_options(&[
        (HTTP_METHOD_GET, "GET"),
        (HTTP_METHOD_POST, "POST"),
        (HTTP_METHOD_PUT, "PUT"),
        (HTTP_METHOD_PATCH, "PATCH"),
        (HTTP_METHOD_DELETE, "DELETE"),
        (HTTP_METHOD_HEAD, "HEAD"),
        (HTTP_METHOD_OPTIONS, "OPTIONS"),
    ])
}

pub(crate) fn http_upload_method_enum_options() -> Vec<ParameterEnumOption> {
    enum_options(&[
        (HTTP_METHOD_POST, "POST"),
        (HTTP_METHOD_PUT, "PUT"),
        (HTTP_METHOD_PATCH, "PATCH"),
    ])
}

pub(crate) fn http_auth_mode_enum_options() -> Vec<ParameterEnumOption> {
    enum_options(&[
        (HTTP_AUTH_NONE, "None"),
        (HTTP_AUTH_BASIC, "Basic"),
        (HTTP_AUTH_BEARER, "Bearer Token"),
        (HTTP_AUTH_HEADER, "Custom Header"),
    ])
}

pub(crate) fn http_body_mode_enum_options() -> Vec<ParameterEnumOption> {
    enum_options(&[
        (HTTP_BODY_NONE, "None"),
        (HTTP_BODY_TEXT, "Text"),
        (HTTP_BODY_JSON, "JSON"),
        (HTTP_BODY_FORM, "Form URL Encoded"),
    ])
}

pub(crate) fn parse_header_lines(text: &str, context: &str) -> Result<Vec<HttpHeader>, String> {
    let mut headers = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let Some(parsed) = parse_optional_key_value_line(index, line) else {
            continue;
        };
        let (name, value) = parsed?;
        validate_header_name(name.as_str(), context)?;
        validate_header_value(value.as_str(), context)?;
        headers.push(HttpHeader { name, value });
    }
    Ok(headers)
}

pub(crate) fn parse_form_field_lines(text: &str, context: &str) -> Result<Vec<HttpFormField>, String> {
    let mut fields = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let Some(parsed) = parse_optional_key_value_line(index, line) else {
            continue;
        };
        let (name, value) = parsed?;
        if name.trim().is_empty() {
            return Err(format!("{context} form field name cannot be empty"));
        }
        fields.push(HttpFormField { name, value });
    }
    Ok(fields)
}

pub(crate) fn response_json_received_values(
    body: &[u8],
    content_type: Option<&str>,
    request_path: &str,
    value_path: &str,
) -> Result<Option<Vec<HttpReceivedValue>>, String> {
    if !response_should_parse_json(body, content_type) {
        return Ok(None);
    }

    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|error| format!("invalid HTTP JSON response: {error}"))?;
    let mut messages = Vec::new();
    let mut path_segments = response_value_path_segments(request_path, value_path);
    let source_description = format!("HTTP response '{}'", response_source_label(request_path));
    collect_json_received_values(
        &value,
        &mut path_segments,
        source_description.as_str(),
        &mut messages,
    )?;
    Ok(Some(messages))
}

pub(crate) fn response_value_path_segments(request_path: &str, value_path: &str) -> Vec<String> {
    let explicit = path_segments(value_path);
    if !explicit.is_empty() {
        return explicit;
    }

    let derived = path_segments(request_path.split(['?', '#']).next().unwrap_or(request_path));
    if derived.is_empty() {
        vec!["response".to_string()]
    } else {
        derived
    }
}

pub(crate) fn response_script_body_arg(body: &[u8]) -> serde_json::Value {
    std::str::from_utf8(body)
        .map(|text| serde_json::json!(text))
        .unwrap_or_else(|_| crate::app::module::script_api::bytes_arg(body))
}

pub(crate) fn response_script_json_arg(body: &[u8]) -> serde_json::Value {
    serde_json::from_slice(body).unwrap_or(serde_json::Value::Null)
}

pub(crate) fn body_looks_like_json(body: &[u8]) -> bool {
    let mut bytes = body.iter().copied().skip_while(u8::is_ascii_whitespace);
    matches!(bytes.next(), Some(b'{') | Some(b'['))
}

pub(crate) fn content_type_is_json(content_type: Option<&str>) -> bool {
    content_type
        .unwrap_or_default()
        .split(';')
        .next()
        .map(str::trim)
        .is_some_and(|media_type| {
            let media_type = media_type.to_ascii_lowercase();
            media_type == "application/json" || media_type.ends_with("+json")
        })
}

fn response_should_parse_json(body: &[u8], content_type: Option<&str>) -> bool {
    content_type_is_json(content_type) || body_looks_like_json(body)
}

fn collect_json_received_values(
    value: &serde_json::Value,
    path_segments: &mut Vec<String>,
    source_description: &str,
    messages: &mut Vec<HttpReceivedValue>,
) -> Result<(), String> {
    if let Some(payload) = decode_json_payload(value)? {
        messages.push(HttpReceivedValue {
            path_segments: path_segments.clone(),
            payload,
            source_description: source_description.to_string(),
        });
        return Ok(());
    }

    match value {
        serde_json::Value::Object(entries) => {
            for (key, child) in entries {
                let key = key.trim();
                if key.is_empty() {
                    continue;
                }
                path_segments.push(key.to_string());
                collect_json_received_values(child, path_segments, source_description, messages)?;
                path_segments.pop();
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                path_segments.push((index + 1).to_string());
                collect_json_received_values(child, path_segments, source_description, messages)?;
                path_segments.pop();
            }
        }
        _ => {}
    }

    Ok(())
}

fn decode_json_payload(value: &serde_json::Value) -> Result<Option<ReceivedValuePayload>, String> {
    match value {
        serde_json::Value::Object(_) => Ok(None),
        serde_json::Value::Array(items) => {
            if let Ok(value) = ParamValue::from_script_json(value) {
                return Ok(Some(ReceivedValuePayload::Single(value)));
            }

            let mut values = Vec::with_capacity(items.len());
            for item in items {
                let Ok(value) = ParamValue::from_script_json(item) else {
                    return Ok(None);
                };
                values.push(value);
            }

            Ok(Some(match values.len() {
                0 => ReceivedValuePayload::Single(ParamValue::Trigger()),
                1 => ReceivedValuePayload::Single(values.remove(0)),
                _ => ReceivedValuePayload::Multi(values),
            }))
        }
        _ => ParamValue::from_script_json(value)
            .map(ReceivedValuePayload::Single)
            .map(Some)
            .map_err(|error| format!("invalid JSON value: {error}")),
    }
}

fn parse_optional_key_value_line(
    index: usize,
    line: &str,
) -> Option<Result<(String, String), String>> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let delimiter = trimmed.find(':').or_else(|| trimmed.find('='));
    Some(match delimiter {
        Some(delimiter) => {
            let name = trimmed[..delimiter].trim();
            let value = trimmed[delimiter + 1..].trim();
            if name.is_empty() {
                Err(format!("line {} has an empty name", index + 1))
            } else {
                Ok((name.to_string(), value.to_string()))
            }
        }
        None => Err(format!("line {} must use 'Name: Value' or 'name=value'", index + 1)),
    })
}

fn validate_header_name(name: &str, context: &str) -> Result<(), String> {
    reqwest::header::HeaderName::from_bytes(name.as_bytes())
        .map(|_| ())
        .map_err(|error| format!("{context} header name '{name}' is invalid: {error}"))
}

fn validate_header_value(value: &str, context: &str) -> Result<(), String> {
    reqwest::header::HeaderValue::from_str(value)
        .map(|_| ())
        .map_err(|error| format!("{context} header value is invalid: {error}"))
}

fn path_segments(path: &str) -> Vec<String> {
    path.trim()
        .trim_matches('/')
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect()
}

fn response_source_label(request_path: &str) -> String {
    let trimmed = request_path.trim();
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalized_variant(variant: &str) -> String {
    variant.trim().to_ascii_lowercase().replace('-', "_")
}

fn enum_options(options: &[(&str, &str)]) -> Vec<ParameterEnumOption> {
    options
        .iter()
        .enumerate()
        .map(|(ordering, (variant_id, label))| ParameterEnumOption {
            variant_id: (*variant_id).to_string(),
            value: ParamValue::Enum((*variant_id).to_string()),
            label: (*label).to_string(),
            tags: Vec::new(),
            ordering: Some(ordering as i32),
        })
        .collect()
}
