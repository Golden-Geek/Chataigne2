use super::{SubtitleError, ffmpeg, timeline::MAX_CUE_BYTES};
use std::ffi::{CStr, c_char};

const MAX_RECTS: usize = 8;
const MAX_DECODED_RECT_BYTES: usize = 256 * 1024;

pub(super) struct SubtitleOutput(pub ffmpeg::Subtitle);

impl Drop for SubtitleOutput {
    fn drop(&mut self) {
        // SAFETY: the zero-initialized ffmpeg-next value has no Drop and exclusively owns
        // these decoder allocations. FFmpeg also permits freeing empty/partial output.
        unsafe { ffmpeg::ffi::avsubtitle_free(self.0.as_mut_ptr()) };
    }
}

impl SubtitleOutput {
    pub fn plain_dialogue(&self) -> Result<Option<String>, SubtitleError> {
        let rectangles = self.0.rects();
        if rectangles.len() > MAX_RECTS {
            return Err(SubtitleError::LimitExceeded);
        }
        let mut text = String::new();
        for rectangle in rectangles {
            let part = match rectangle {
                ffmpeg::subtitle::Rect::Ass(rect) => {
                    // SAFETY: The live subtitle owner retains the decoder-returned rectangle.
                    let raw = unsafe { checked_text((*rect.as_ptr()).ass, self) }?;
                    ass_dialogue(raw)?
                }
                ffmpeg::subtitle::Rect::Text(rect) => {
                    // SAFETY: The live subtitle owner retains the decoder-returned rectangle.
                    unsafe { checked_text((*rect.as_ptr()).text, self) }?.to_owned()
                }
                ffmpeg::subtitle::Rect::None(_) | ffmpeg::subtitle::Rect::Bitmap(_) => continue,
            };
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if !text.is_empty() {
                text.push('\n');
            }
            if text.len() + part.len() > MAX_CUE_BYTES {
                return Err(SubtitleError::LimitExceeded);
            }
            text.push_str(part);
        }
        Ok((!text.is_empty()).then_some(text))
    }
}

// The pointer must be a NUL-terminated FFmpeg string retained by the supplied owner.
unsafe fn checked_text(raw: *const c_char, _owner: &SubtitleOutput) -> Result<&str, SubtitleError> {
    if raw.is_null() {
        return Err(SubtitleError::InvalidText);
    }
    // SAFETY: callers use only the FFmpeg-owned NUL-terminated fields of a live rectangle.
    let text = unsafe { CStr::from_ptr(raw) };
    if text.to_bytes().len() > MAX_DECODED_RECT_BYTES {
        return Err(SubtitleError::LimitExceeded);
    }
    text.to_str().map_err(|_| SubtitleError::InvalidText)
}

pub(super) fn ass_dialogue(line: &str) -> Result<String, SubtitleError> {
    // libavcodec emits the Matroska ASS event layout for the other text codecs as well as ASS.
    // Its eight metadata fields precede the text, which can contain arbitrary commas.
    let dialogue = line.splitn(9, ',').nth(8).ok_or(SubtitleError::InvalidText)?;
    let mut remaining = dialogue;
    let mut text = String::new();
    let mut drawing = false;
    while let Some(ch) = remaining.chars().next() {
        remaining = &remaining[ch.len_utf8()..];
        if ch == '{'
            && let Some((tags, rest)) = remaining.split_once('}')
            && tags.starts_with('\\')
        {
            for tag in tags.split('\\').filter_map(|tag| tag.strip_prefix('p')) {
                if let Some(digit) = tag.as_bytes().first().filter(|digit| digit.is_ascii_digit()) {
                    drawing = *digit != b'0';
                }
            }
            remaining = rest;
            continue;
        }
        if ch == '\\'
            && let Some(escape) = remaining.chars().next()
            && let Some(decoded) = match escape {
                'N' | 'n' => Some('\n'),
                'h' => Some('\u{00a0}'),
                '{' | '}' | '\\' => Some(escape),
                _ => None,
            }
        {
            remaining = &remaining[escape.len_utf8()..];
            if !drawing {
                text.push(decoded);
            }
        } else if !drawing {
            text.push(ch);
        }
        if text.len() > MAX_CUE_BYTES {
            return Err(SubtitleError::LimitExceeded);
        }
    }
    Ok(text)
}
