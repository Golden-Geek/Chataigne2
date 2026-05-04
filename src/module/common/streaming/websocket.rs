use std::{
    collections::HashMap,
    io::{Error, ErrorKind, Read, Write},
    net::TcpStream,
};

const HTTP_MAX_HEADER_BYTES: usize = 32 * 1024;
pub(crate) const WEBSOCKET_MAX_PAYLOAD_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WebSocketFrameKind {
    Text,
    Binary,
}

impl WebSocketFrameKind {
    fn opcode(self) -> u8 {
        match self {
            Self::Text => 0x1,
            Self::Binary => 0x2,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WebSocketIncomingFrame {
    Text(String),
    Binary(Vec<u8>),
    Close,
    Ping(Vec<u8>),
    Pong(Vec<u8>),
}

#[derive(Clone, Debug)]
pub(crate) struct WebSocketHttpRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WebSocketReadStatus {
    Open,
    Closed,
}

struct WebSocketHttpResponse {
    status_code: u16,
    headers: HashMap<String, String>,
}

pub(crate) fn normalize_websocket_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        "/".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

pub(crate) fn perform_client_handshake(
    stream: &mut TcpStream,
    remote_host: &str,
    remote_port: u16,
    path: &str,
) -> std::io::Result<()> {
    let path = normalize_websocket_path(path);
    let key = base64_encode(&rand::random::<[u8; 16]>());
    let request = format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {remote_host}:{remote_port}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Version: 13\r\n\
         Sec-WebSocket-Key: {key}\r\n\r\n"
    );

    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    let response = read_http_response(stream)?;
    if response.status_code != 101 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("websocket upgrade failed with HTTP {}", response.status_code),
        ));
    }
    if !header_contains_token(&response.headers, "connection", "upgrade") {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "websocket upgrade response is missing 'Connection: Upgrade'",
        ));
    }
    if !response
        .headers
        .get("upgrade")
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
    {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "websocket upgrade response is missing 'Upgrade: websocket'",
        ));
    }

    let expected_accept = websocket_accept_key(key.as_str());
    let actual_accept = response
        .headers
        .get("sec-websocket-accept")
        .map(String::as_str)
        .unwrap_or_default();
    if actual_accept.trim() != expected_accept {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "websocket upgrade response returned an invalid accept key",
        ));
    }

    Ok(())
}

pub(crate) fn read_http_request(stream: &mut TcpStream) -> std::io::Result<WebSocketHttpRequest> {
    let head = read_http_head(stream)?;
    let header_text = std::str::from_utf8(head.as_slice())
        .map_err(|error| Error::new(ErrorKind::InvalidData, format!("invalid header utf-8: {error}")))?;

    let mut lines = header_text.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing request line"))?;

    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing request method"))?
        .to_string();
    let target = parts
        .next()
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing request target"))?;

    Ok(WebSocketHttpRequest {
        method,
        path: target.split('?').next().unwrap_or(target).to_string(),
        headers: parse_headers(lines)?,
    })
}

pub(crate) fn is_websocket_upgrade_request(request: &WebSocketHttpRequest) -> bool {
    request.method.eq_ignore_ascii_case("GET")
        && header_contains_token(&request.headers, "connection", "upgrade")
        && request
            .headers
            .get("upgrade")
            .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
        && request.headers.contains_key("sec-websocket-key")
}

pub(crate) fn accept_server_handshake(
    stream: &mut TcpStream,
    request: &WebSocketHttpRequest,
) -> std::io::Result<()> {
    let version = request
        .headers
        .get("sec-websocket-version")
        .map(String::as_str)
        .unwrap_or_default();
    if version.trim() != "13" {
        write_http_error_response(stream, "426 Upgrade Required", "unsupported websocket version")?;
        return Err(Error::new(
            ErrorKind::InvalidData,
            "unsupported websocket version",
        ));
    }

    let key = request
        .headers
        .get("sec-websocket-key")
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing sec-websocket-key"))?;
    let accept = websocket_accept_key(key.as_str());
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept}\r\n\r\n"
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

pub(crate) fn write_http_error_response(
    stream: &mut TcpStream,
    status: &str,
    message: &str,
) -> std::io::Result<()> {
    let body = message.as_bytes();
    let response = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );

    stream.write_all(response.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

pub(crate) fn read_available_bytes(
    stream: &mut TcpStream,
    pending: &mut Vec<u8>,
) -> std::io::Result<WebSocketReadStatus> {
    let mut buffer = [0u8; 8192];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => return Ok(WebSocketReadStatus::Closed),
            Ok(length) => {
                pending.extend_from_slice(&buffer[..length]);
                if pending.len() > WEBSOCKET_MAX_PAYLOAD_BYTES + 14 {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "websocket pending buffer exceeds payload limit",
                    ));
                }
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(WebSocketReadStatus::Open),
            Err(error) => return Err(error),
        }
    }
}

pub(crate) fn try_take_frame(
    pending: &mut Vec<u8>,
    expect_masked: bool,
) -> std::io::Result<Option<WebSocketIncomingFrame>> {
    if pending.len() < 2 {
        return Ok(None);
    }

    let header0 = pending[0];
    let header1 = pending[1];
    let fin = (header0 & 0x80) != 0;
    if !fin {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "fragmented websocket frames are not supported",
        ));
    }

    let masked = (header1 & 0x80) != 0;
    if masked != expect_masked {
        return Err(Error::new(
            ErrorKind::InvalidData,
            if expect_masked {
                "client websocket frames must be masked"
            } else {
                "server websocket frames must not be masked"
            },
        ));
    }

    let opcode = header0 & 0x0F;
    let mut cursor = 2usize;
    let mut payload_len = usize::from(header1 & 0x7F);

    if payload_len == 126 {
        if pending.len() < cursor + 2 {
            return Ok(None);
        }
        payload_len = usize::from(u16::from_be_bytes([pending[cursor], pending[cursor + 1]]));
        cursor += 2;
    } else if payload_len == 127 {
        if pending.len() < cursor + 8 {
            return Ok(None);
        }
        let extended = u64::from_be_bytes([
            pending[cursor],
            pending[cursor + 1],
            pending[cursor + 2],
            pending[cursor + 3],
            pending[cursor + 4],
            pending[cursor + 5],
            pending[cursor + 6],
            pending[cursor + 7],
        ]);
        payload_len = usize::try_from(extended)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "websocket payload exceeds platform limits"))?;
        cursor += 8;
    }

    if payload_len > WEBSOCKET_MAX_PAYLOAD_BYTES {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "websocket frame exceeds payload limit",
        ));
    }

    let mask = if masked {
        if pending.len() < cursor + 4 {
            return Ok(None);
        }
        let mask = [
            pending[cursor],
            pending[cursor + 1],
            pending[cursor + 2],
            pending[cursor + 3],
        ];
        cursor += 4;
        Some(mask)
    } else {
        None
    };

    let frame_end = cursor + payload_len;
    if pending.len() < frame_end {
        return Ok(None);
    }

    let mut payload = pending[cursor..frame_end].to_vec();
    if let Some(mask) = mask {
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[index % 4];
        }
    }
    pending.drain(..frame_end);

    let frame = match opcode {
        0x1 => {
            let text = std::str::from_utf8(payload.as_slice()).map_err(|error| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid websocket text payload: {error}"),
                )
            })?;
            WebSocketIncomingFrame::Text(text.to_string())
        }
        0x2 => WebSocketIncomingFrame::Binary(payload),
        0x8 => WebSocketIncomingFrame::Close,
        0x9 => WebSocketIncomingFrame::Ping(payload),
        0xA => WebSocketIncomingFrame::Pong(payload),
        _ => {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("unsupported websocket opcode: {opcode}"),
            ));
        }
    };

    Ok(Some(frame))
}

pub(crate) fn write_data_frame(
    stream: &mut TcpStream,
    frame_kind: WebSocketFrameKind,
    payload: &[u8],
    masked: bool,
) -> std::io::Result<()> {
    if matches!(frame_kind, WebSocketFrameKind::Text) {
        std::str::from_utf8(payload).map_err(|error| {
            Error::new(
                ErrorKind::InvalidData,
                format!("text websocket frame requires valid UTF-8 payload: {error}"),
            )
        })?;
    }

    write_frame(stream, frame_kind.opcode(), payload, masked)
}

pub(crate) fn write_control_frame(
    stream: &mut TcpStream,
    opcode: u8,
    payload: &[u8],
    masked: bool,
) -> std::io::Result<()> {
    write_frame(stream, opcode, payload, masked)
}

fn read_http_response(stream: &mut TcpStream) -> std::io::Result<WebSocketHttpResponse> {
    let head = read_http_head(stream)?;
    let header_text = std::str::from_utf8(head.as_slice())
        .map_err(|error| Error::new(ErrorKind::InvalidData, format!("invalid header utf-8: {error}")))?;

    let mut lines = header_text.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing response status line"))?;
    let mut parts = status_line.split_whitespace();
    let _http_version = parts
        .next()
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing HTTP version"))?;
    let status_code = parts
        .next()
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing HTTP status code"))?
        .parse::<u16>()
        .map_err(|error| Error::new(ErrorKind::InvalidData, format!("invalid HTTP status code: {error}")))?;

    Ok(WebSocketHttpResponse {
        status_code,
        headers: parse_headers(lines)?,
    })
}

fn read_http_head(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut buffer = Vec::with_capacity(2048);
    let mut temp = [0u8; 1024];

    loop {
        let read = stream.read(&mut temp)?;
        if read == 0 {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "peer disconnected before sending HTTP headers",
            ));
        }

        buffer.extend_from_slice(&temp[..read]);
        if buffer.len() > HTTP_MAX_HEADER_BYTES {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("HTTP headers exceed {HTTP_MAX_HEADER_BYTES} bytes"),
            ));
        }

        if let Some(index) = find_header_end(buffer.as_slice()) {
            return Ok(buffer[..index].to_vec());
        }
    }
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_headers<'a, I>(lines: I) -> std::io::Result<HashMap<String, String>>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut headers = HashMap::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }

        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "malformed HTTP header line"))?;
        let key = name.trim().to_ascii_lowercase();
        let value = value.trim();
        headers
            .entry(key)
            .and_modify(|existing: &mut String| {
                existing.push(',');
                existing.push_str(value);
            })
            .or_insert_with(|| value.to_string());
    }
    Ok(headers)
}

fn header_contains_token(headers: &HashMap<String, String>, header_name: &str, expected_token: &str) -> bool {
    headers.get(header_name).is_some_and(|value| {
        value
            .split(',')
            .any(|part| part.trim().eq_ignore_ascii_case(expected_token))
    })
}

fn write_frame(stream: &mut TcpStream, opcode: u8, payload: &[u8], masked: bool) -> std::io::Result<()> {
    if payload.len() > WEBSOCKET_MAX_PAYLOAD_BYTES {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "websocket frame exceeds payload limit",
        ));
    }

    let mut frame = Vec::with_capacity(14 + payload.len());
    frame.push(0x80 | (opcode & 0x0F));

    let mask_bit = if masked { 0x80 } else { 0x00 };
    if payload.len() < 126 {
        frame.push(mask_bit | payload.len() as u8);
    } else if payload.len() <= u16::MAX as usize {
        frame.push(mask_bit | 126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        frame.push(mask_bit | 127);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }

    if masked {
        let mask = rand::random::<[u8; 4]>();
        frame.extend_from_slice(mask.as_slice());

        let mut masked_payload = payload.to_vec();
        for (index, byte) in masked_payload.iter_mut().enumerate() {
            *byte ^= mask[index % 4];
        }
        frame.extend_from_slice(masked_payload.as_slice());
    } else {
        frame.extend_from_slice(payload);
    }

    stream.write_all(frame.as_slice())?;
    stream.flush()?;
    Ok(())
}

fn websocket_accept_key(key: &str) -> String {
    let mut input = Vec::<u8>::new();
    input.extend_from_slice(key.trim().as_bytes());
    input.extend_from_slice(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let digest = sha1_digest(input.as_slice());
    base64_encode(digest.as_slice())
}

fn sha1_digest(input: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x6745_2301;
    let mut h1: u32 = 0xEFCD_AB89;
    let mut h2: u32 = 0x98BA_DCFE;
    let mut h3: u32 = 0x1032_5476;
    let mut h4: u32 = 0xC3D2_E1F0;

    let bit_len = (input.len() as u64) * 8;
    let mut message = input.to_vec();
    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    message.extend_from_slice(bit_len.to_be_bytes().as_slice());

    for chunk in message.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (index, word) in w.iter_mut().take(16).enumerate() {
            let base = index * 4;
            *word = u32::from_be_bytes([chunk[base], chunk[base + 1], chunk[base + 2], chunk[base + 3]]);
        }
        for index in 16..80 {
            w[index] = (w[index - 3] ^ w[index - 8] ^ w[index - 14] ^ w[index - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (index, word) in w.iter().enumerate() {
            let (f, k) = match index {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };

            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut output = [0u8; 20];
    output[0..4].copy_from_slice(h0.to_be_bytes().as_slice());
    output[4..8].copy_from_slice(h1.to_be_bytes().as_slice());
    output[8..12].copy_from_slice(h2.to_be_bytes().as_slice());
    output[12..16].copy_from_slice(h3.to_be_bytes().as_slice());
    output[16..20].copy_from_slice(h4.to_be_bytes().as_slice());
    output
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut output = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut index = 0usize;

    while index + 3 <= data.len() {
        let chunk = ((data[index] as u32) << 16) | ((data[index + 1] as u32) << 8) | data[index + 2] as u32;
        output.push(TABLE[((chunk >> 18) & 0x3F) as usize] as char);
        output.push(TABLE[((chunk >> 12) & 0x3F) as usize] as char);
        output.push(TABLE[((chunk >> 6) & 0x3F) as usize] as char);
        output.push(TABLE[(chunk & 0x3F) as usize] as char);
        index += 3;
    }

    let remainder = data.len() - index;
    if remainder == 1 {
        let chunk = (data[index] as u32) << 16;
        output.push(TABLE[((chunk >> 18) & 0x3F) as usize] as char);
        output.push(TABLE[((chunk >> 12) & 0x3F) as usize] as char);
        output.push('=');
        output.push('=');
    } else if remainder == 2 {
        let chunk = ((data[index] as u32) << 16) | ((data[index + 1] as u32) << 8);
        output.push(TABLE[((chunk >> 18) & 0x3F) as usize] as char);
        output.push(TABLE[((chunk >> 12) & 0x3F) as usize] as char);
        output.push(TABLE[((chunk >> 6) & 0x3F) as usize] as char);
        output.push('=');
    }

    output
}