use super::*;
use serde_json::Value;
use std::mem::size_of;

pub(in super::super) fn request_weight(request: &HttpQueuedRequest) -> Result<usize, String> {
    Ok(size_of::<HttpQueuedRequest>() + payload_weight(&request.payload)? + origin_weight(&request.origin))
}

pub(super) fn event_weight(event: &HttpOutcome) -> Result<usize, String> {
    let (origin, payload) = match event {
        HttpOutcome::Response(response) => (&response.origin, &response.request),
        HttpOutcome::Failure(failure) => (&failure.origin, &failure.request),
    };
    // Requests were bounded at admission. Borrow their metadata without cloning large bodies.
    let mut weight = payload_weight(payload)? + origin_weight(origin) + size_of::<HttpOutcome>();
    match event {
        HttpOutcome::Failure(failure) => {
            weight += failure.error.capacity();
        }
        HttpOutcome::Response(response) => {
            weight += response.body.capacity()
                + response.url.capacity()
                + response.status_text.capacity()
                + headers_weight(&response.headers)
                + response.content_type.as_ref().map_or(0, String::capacity);
            weight += response.received_values.as_ref().err().map_or(0, String::capacity);
            if let Ok(Some(values)) = &response.received_values {
                weight += values.capacity() * size_of::<ReceivedValueMessage>();
                for value in values {
                    weight = weight.saturating_add(value.retained_weight());
                }
            }
        }
    }
    Ok(weight)
}

fn payload_weight(payload: &HttpRequestPayload) -> Result<usize, String> {
    if payload.headers.len() > 256 {
        return Err("HTTP requests support at most 256 headers".into());
    }
    let mut weight = size_of::<HttpRequestPayload>()
        + payload.path.capacity()
        + payload.query.capacity()
        + payload.value_path.capacity()
        + payload.description.capacity()
        + headers_weight(&payload.headers);
    weight += match &payload.body {
        HttpRequestBody::Empty => 0,
        HttpRequestBody::Text { text, content_type } => text.capacity() + content_type.capacity(),
        HttpRequestBody::Json { value } => json_weight(value)?,
        HttpRequestBody::Form { fields } | HttpRequestBody::Multipart { fields, .. } => {
            if fields.len() > 256 {
                return Err("HTTP requests support at most 256 form fields".into());
            }
            let mut bytes = fields.capacity() * size_of::<crate::app::module::common::http::HttpFormField>();
            for field in fields {
                bytes += field.name.capacity() + field.value.capacity();
            }
            if let HttpRequestBody::Multipart { files, .. } = &payload.body {
                if files.len() > 16 {
                    return Err("HTTP requests support at most 16 files".into());
                }
                bytes += files.capacity() * size_of::<crate::app::module::common::http::HttpFilePart>();
                for file in files {
                    bytes += file.field_name.capacity()
                        + file.path.capacity()
                        + file.file_name.as_ref().map_or(0, String::capacity)
                        + file.content_type.as_ref().map_or(0, String::capacity);
                }
            }
            bytes
        }
    };
    Ok(weight)
}

fn headers_weight(headers: &Vec<HttpHeader>) -> usize {
    headers.capacity() * size_of::<HttpHeader>()
        + headers
            .iter()
            .map(|header| header.name.capacity() + header.value.capacity())
            .sum::<usize>()
}

fn origin_weight(origin: &HttpRequestOrigin) -> usize {
    match origin {
        HttpRequestOrigin::Script { method } => method.capacity(),
        HttpRequestOrigin::Command { command_type, .. } => command_type.capacity(),
    }
}

pub(super) fn json_weight(value: &Value) -> Result<usize, String> {
    fn visit(value: &Value, nodes: &mut usize, depth: usize) -> Result<usize, String> {
        *nodes += 1;
        if *nodes > 16_384 || depth > 64 {
            return Err("HTTP JSON exceeds the 16384-value / 64-level limit".into());
        }
        let mut bytes = size_of::<Value>();
        match value {
            Value::String(value) => bytes += value.capacity(),
            Value::Array(values) => {
                bytes += values.capacity() * size_of::<Value>();
                for value in values {
                    bytes = bytes.saturating_add(visit(value, nodes, depth + 1)?);
                }
            }
            Value::Object(values) => {
                // Include map storage overhead as well as keys and child values.
                bytes += values.len() * (size_of::<String>() + size_of::<Value>() + 4 * size_of::<usize>());
                for (key, value) in values {
                    bytes = bytes.saturating_add(key.capacity() + visit(value, nodes, depth + 1)?);
                }
            }
            _ => {}
        }
        Ok(bytes)
    }
    visit(value, &mut 0, 0)
}
