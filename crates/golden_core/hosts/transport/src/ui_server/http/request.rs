use super::*;

const MAX_HEADERS: usize = 128;

pub(in crate::ui_server) fn read_http_request(stream: &mut TcpStream) -> std::io::Result<HttpRequest> {
    let mut buffer = Vec::with_capacity(2_048);
    let mut chunk = [0; 1_024];
    let mut head = None;
    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(Error::new(ErrorKind::UnexpectedEof, "HTTP request was truncated"));
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > HTTP_MAX_REQUEST_BYTES {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "HTTP request exceeds its byte limit",
            ));
        }
        if head.is_none() {
            if let Some(offset) = buffer.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                head = Some(parse_head(&buffer[..offset + 4])?);
            } else if buffer.len() > HTTP_MAX_HEADER_BYTES {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "HTTP headers exceed their byte limit",
                ));
            }
        }
        if let Some((request, offset, length)) = head.as_mut()
            && buffer.len() >= *offset + *length
        {
            // The host closes each HTTP connection; pipelined bytes never become a second request.
            buffer.drain(..*offset);
            buffer.truncate(*length);
            request.body = buffer;
            return Ok(head.unwrap().0);
        }
    }
}

pub(super) fn parse_head(bytes: &[u8]) -> std::io::Result<(HttpRequest, usize, usize)> {
    let mut fields = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let mut parsed = httparse::Request::new(&mut fields);
    let offset = match parsed
        .parse(bytes)
        .map_err(|error| Error::new(ErrorKind::InvalidData, error))?
    {
        httparse::Status::Complete(offset) => offset,
        httparse::Status::Partial => return Err(Error::new(ErrorKind::InvalidData, "incomplete HTTP headers")),
    };
    if parsed.version != Some(1) {
        return Err(Error::new(ErrorKind::InvalidData, "HTTP/1.1 is required"));
    }
    let method = parsed
        .method
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing HTTP method"))?;
    let target = parsed
        .path
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing request target"))?;
    if !target.starts_with('/') || target.starts_with("//") || target.contains('#') {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "HTTP target must be an origin-form path",
        ));
    }
    let mut headers = HashMap::<String, String>::new();
    for field in parsed.headers {
        let name = field.name.to_ascii_lowercase();
        let value = std::str::from_utf8(field.value)
            .map_err(|error| Error::new(ErrorKind::InvalidData, error))?
            .trim();
        if let Some(previous) = headers.get_mut(&name) {
            if matches!(
                name.as_str(),
                "host" | "origin" | "content-length" | "authorization" | "sec-websocket-key" | "sec-websocket-version"
            ) {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "ambiguous duplicate HTTP authority, framing or credential header",
                ));
            }
            previous.push(',');
            previous.push_str(value);
        } else {
            headers.insert(name, value.to_owned());
        }
    }
    if headers.contains_key("transfer-encoding") {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "transfer encoding is not supported; use Content-Length",
        ));
    }
    let length =
        match headers.get("content-length") {
            None => 0,
            Some(value) if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => value
                .parse::<usize>()
                .map_err(|error| Error::new(ErrorKind::InvalidData, error))?,
            Some(_) => return Err(Error::new(ErrorKind::InvalidData, "invalid Content-Length")),
        };
    validate_http_request_size(offset, length)?;
    Ok((
        HttpRequest {
            method: method.to_owned(),
            path: target.split('?').next().unwrap_or(target).to_owned(),
            headers,
            body: Vec::new(),
        },
        offset,
        length,
    ))
}

pub(in crate::ui_server) fn validate_http_request_size(
    header_bytes: usize,
    content_length: usize,
) -> std::io::Result<()> {
    if header_bytes > HTTP_MAX_HEADER_BYTES
        || header_bytes
            .checked_add(content_length)
            .is_none_or(|total| total > HTTP_MAX_REQUEST_BYTES)
    {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "HTTP request exceeds its header or total byte limit",
        ));
    }
    Ok(())
}
