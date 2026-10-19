use crate::app::module::common::http::{
    parse_header_lines, HttpFilePart, HttpHeader, HttpMethod, HttpRequestBody, HttpRequestPayload,
    HTTP_TEXT_CONTENT_TYPE,
};
use golden_core::parameter::ParamValue;

pub(super) fn http_request_from_script(
    method: &str,
    args: &[ParamValue],
) -> Option<Result<HttpRequestPayload, String>> {
    match method {
        "request" => Some(script_request(args)),
        "requestJson" => Some(script_json_request(args)),
        "get" => Some(script_empty_body_request(HttpMethod::Get, method, args)),
        "head" => Some(script_empty_body_request(HttpMethod::Head, method, args)),
        "options" => Some(script_empty_body_request(HttpMethod::Options, method, args)),
        "delete" => Some(script_empty_body_request(HttpMethod::Delete, method, args)),
        "post" => Some(script_text_body_request(HttpMethod::Post, method, args)),
        "put" => Some(script_text_body_request(HttpMethod::Put, method, args)),
        "patch" => Some(script_text_body_request(HttpMethod::Patch, method, args)),
        "postJson" => Some(script_fixed_json_request(HttpMethod::Post, method, args)),
        "putJson" => Some(script_fixed_json_request(HttpMethod::Put, method, args)),
        "patchJson" => Some(script_fixed_json_request(HttpMethod::Patch, method, args)),
        "uploadFile" => Some(script_upload_file_request(args)),
        _ => None,
    }
}

fn script_request(args: &[ParamValue]) -> Result<HttpRequestPayload, String> {
    let method = required_script_string(args, 0, "request method")?;
    let method = HttpMethod::from_variant(method.as_str()).ok_or_else(|| format!("invalid HTTP method '{method}'"))?;
    let path = required_script_string(args, 1, "request path")?;
    let body = if args.get(2).is_some() {
        HttpRequestBody::Text {
            text: args.get(2).and_then(ParamValue::as_str).unwrap_or_default(),
            content_type: script_content_type(args.get(3)),
        }
    } else {
        HttpRequestBody::Empty
    };

    Ok(HttpRequestPayload {
        method,
        path,
        query: String::new(),
        value_path: optional_script_string(args, 5),
        headers: script_headers(args.get(4))?,
        body,
        description: "script request".to_string(),
    })
}

fn script_json_request(args: &[ParamValue]) -> Result<HttpRequestPayload, String> {
    let method = required_script_string(args, 0, "request method")?;
    let method = HttpMethod::from_variant(method.as_str()).ok_or_else(|| format!("invalid HTTP method '{method}'"))?;
    let path = required_script_string(args, 1, "request path")?;
    let value = args
        .get(2)
        .map(ParamValue::to_script_json)
        .unwrap_or(serde_json::Value::Null);

    Ok(HttpRequestPayload {
        method,
        path,
        query: String::new(),
        value_path: optional_script_string(args, 4),
        headers: script_headers(args.get(3))?,
        body: HttpRequestBody::Json { value },
        description: "script JSON request".to_string(),
    })
}

fn script_empty_body_request(
    method: HttpMethod,
    script_method: &str,
    args: &[ParamValue],
) -> Result<HttpRequestPayload, String> {
    Ok(HttpRequestPayload {
        method,
        path: required_script_string(args, 0, "request path")?,
        query: String::new(),
        value_path: optional_script_string(args, 2),
        headers: script_headers(args.get(1))?,
        body: HttpRequestBody::Empty,
        description: script_method.to_string(),
    })
}

fn script_text_body_request(
    method: HttpMethod,
    script_method: &str,
    args: &[ParamValue],
) -> Result<HttpRequestPayload, String> {
    Ok(HttpRequestPayload {
        method,
        path: required_script_string(args, 0, "request path")?,
        query: String::new(),
        value_path: optional_script_string(args, 4),
        headers: script_headers(args.get(3))?,
        body: HttpRequestBody::Text {
            text: optional_script_string(args, 1),
            content_type: script_content_type(args.get(2)),
        },
        description: script_method.to_string(),
    })
}

fn script_fixed_json_request(
    method: HttpMethod,
    script_method: &str,
    args: &[ParamValue],
) -> Result<HttpRequestPayload, String> {
    Ok(HttpRequestPayload {
        method,
        path: required_script_string(args, 0, "request path")?,
        query: String::new(),
        value_path: optional_script_string(args, 3),
        headers: script_headers(args.get(2))?,
        body: HttpRequestBody::Json {
            value: args
                .get(1)
                .map(ParamValue::to_script_json)
                .unwrap_or(serde_json::Value::Null),
        },
        description: script_method.to_string(),
    })
}

fn script_upload_file_request(args: &[ParamValue]) -> Result<HttpRequestPayload, String> {
    let method = required_script_string(args, 0, "upload method")?;
    let method =
        HttpMethod::from_variant(method.as_str()).ok_or_else(|| format!("invalid HTTP upload method '{method}'"))?;
    if !matches!(method, HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch) {
        return Err("uploadFile only supports POST, PUT, and PATCH".to_string());
    }

    let path = required_script_string(args, 1, "request path")?;
    let file_path = required_script_string(args, 2, "file path")?;
    let field_name = optional_script_string(args, 3);
    let field_name = if field_name.trim().is_empty() {
        "file".to_string()
    } else {
        field_name
    };
    let content_type = optional_script_string(args, 4);

    Ok(HttpRequestPayload {
        method,
        path,
        query: String::new(),
        value_path: optional_script_string(args, 6),
        headers: script_headers(args.get(5))?,
        body: HttpRequestBody::Multipart {
            fields: Vec::new(),
            files: vec![HttpFilePart {
                field_name,
                path: file_path,
                file_name: None,
                content_type: (!content_type.trim().is_empty()).then_some(content_type),
            }],
        },
        description: "script file upload".to_string(),
    })
}

fn required_script_string(args: &[ParamValue], index: usize, label: &str) -> Result<String, String> {
    args.get(index)
        .and_then(ParamValue::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("HTTP script method expects a non-empty {label}"))
}

fn optional_script_string(args: &[ParamValue], index: usize) -> String {
    args.get(index).and_then(ParamValue::as_str).unwrap_or_default()
}

fn script_headers(value: Option<&ParamValue>) -> Result<Vec<HttpHeader>, String> {
    let Some(value) = value.and_then(ParamValue::as_str) else {
        return Ok(Vec::new());
    };
    parse_header_lines(value.as_str(), "HTTP script")
}

fn script_content_type(value: Option<&ParamValue>) -> String {
    value
        .and_then(ParamValue::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| HTTP_TEXT_CONTENT_TYPE.to_string())
}
