//! Worker-side subtitle coverage prepared through a private Windows memory DC.

use std::ffi::c_void;
use std::ptr::NonNull;

use thiserror::Error;
use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS, CreateCompatibleDC,
    CreateDIBSection, CreateFontW, DEFAULT_CHARSET, DIB_RGB_COLORS, DT_CALCRECT, DT_CENTER, DT_EDITCONTROL,
    DT_NOPREFIX, DT_WORDBREAK, DeleteDC, DeleteObject, DrawTextW, FW_NORMAL, GdiFlush, GetTextFaceW, HBITMAP, HDC,
    HFONT, LF_FACESIZE, OUT_TT_PRECIS, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::core::{PCWSTR, w};

use super::{ImageData, ImageError};
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};

const MAX_TEXT_BYTES: usize = 4096;
const MAX_CANVAS_PIXELS: usize = 1920 * 1080;
const FALLBACK_FAMILY: PCWSTR = w!("Segoe UI");

/// Normalized subtitle settings independent of the output's physical DPI or extent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubtitleStyle {
    /// Font height as a positive fraction of the canvas height, at most one.
    pub size: f32,
    /// Center X and bottom Y of the wrapped text block, in the unit square.
    pub position: [f32; 2],
}

/// Validation or rasterization failure before an immutable subtitle image is published.
#[derive(Debug, Error)]
pub enum SubtitleRasterError {
    /// The normalized controls are outside their admitted range.
    #[error("invalid subtitle size or position")]
    Style,
    /// The private scratch bitmap would exceed the bounded subtitle raster size.
    #[error("subtitle canvas exceeds 1920×1080 pixels or the native image dimension limit")]
    Canvas,
    /// Cue text exceeds the finite size or contains a NUL character.
    #[error("invalid subtitle text or cue exceeds 4096 UTF-8 bytes")]
    Text,
    /// The system font mapper or offscreen bitmap failed.
    #[error("Windows could not rasterize subtitle text")]
    Graphics,
    /// The immutable output image could not be admitted.
    #[error(transparent)]
    Image(#[from] ImageError),
}

/// Rasterizes plain subtitle cues into transparent immutable sRGB RGBA8.
/// This is cue/style preparation work. Cache the result and run rasterization off UI and
/// renderer submission threads. Native handle ownership stays within each rasterization.
pub struct SubtitleRasterizer {
    family: Option<([u16; LF_FACESIZE as usize], String)>,
}

impl SubtitleRasterizer {
    /// Select a Windows system font by family name. An empty, unavailable or unsupported
    /// name falls back to Segoe UI, with Windows handling any further font substitution.
    pub fn new(family: &str) -> Self {
        let family = family.trim();
        if family.is_empty() || family.len() > 4 * LF_FACESIZE as usize || family.contains('\0') {
            return Self { family: None };
        }
        let mut units = [0; LF_FACESIZE as usize];
        let mut encoded = family.encode_utf16();
        for (target, unit) in units[..LF_FACESIZE as usize - 1].iter_mut().zip(&mut encoded) {
            *target = unit;
        }
        Self {
            family: encoded.next().is_none().then(|| (units, family.to_lowercase())),
        }
    }

    /// Wrap and position a Unicode cue using the fixed logical canvas. The returned image
    /// has the same extent and straight alpha for ordinary source-over composition.
    /// Whitespace-only cues have no image. Long cues clip at the canvas edges.
    pub fn rasterize(
        &self,
        text: &str,
        canvas: Extent2D,
        style: SubtitleStyle,
    ) -> Result<Option<ImageData>, SubtitleRasterError> {
        if !style.size.is_finite()
            || !(0.0 < style.size && style.size <= 1.0)
            || !style.position.iter().all(|value| (0.0..=1.0).contains(value))
        {
            return Err(SubtitleRasterError::Style);
        }
        if text.len() > MAX_TEXT_BYTES || text.contains('\0') {
            return Err(SubtitleRasterError::Text);
        }
        let (width, height) = (canvas.width() as usize, canvas.height() as usize);
        let pixel_count = width.checked_mul(height).ok_or(SubtitleRasterError::Canvas)?;
        if pixel_count > MAX_CANVAS_PIXELS || width > 16_384 || height > 16_384 {
            return Err(SubtitleRasterError::Canvas);
        }
        if text.trim().is_empty() {
            return Ok(None);
        }
        let (width, height) = (width as i32, height as i32);
        let font_px = (height as f32 * style.size).round().max(1.0) as i32;
        let mut target = RasterTarget::new(width, height, pixel_count)?;
        let requested = self.family.as_ref().is_some_and(|(face, family)| {
            target.select_font(PCWSTR(face.as_ptr()), font_px).is_ok() && target.selected_family(family)
        });
        if !requested {
            target.select_font(FALLBACK_FAMILY, font_px)?;
        }
        let mut text: Vec<_> = text.trim().encode_utf16().collect();
        target.draw(&mut text, width, height, style.position)?;
        let pixels = compose_coverage(target.pixels(pixel_count), width as usize)?;
        Ok(Some(ImageData::new(
            TextureDescriptor {
                extent: canvas,
                format: PixelFormat::Rgba8Unorm,
                color_encoding: ColorEncoding::Srgb,
                alpha_mode: AlphaMode::Straight,
            },
            pixels,
        )?))
    }
}

struct RasterTarget {
    dc: HDC,
    bitmap: HBITMAP,
    font: HFONT,
    bits: NonNull<u32>,
}

impl RasterTarget {
    fn new(width: i32, height: i32, pixel_count: usize) -> Result<Self, SubtitleRasterError> {
        // SAFETY: all handles are private to this invocation. A negative DIB height and a
        // 32-bit BI_RGB layout give exactly width*height contiguous BGRA words.
        unsafe {
            let dc = CreateCompatibleDC(None);
            if dc.is_invalid() {
                return Err(SubtitleRasterError::Graphics);
            }
            let mut target = Self {
                dc,
                bitmap: HBITMAP::default(),
                font: HFONT::default(),
                bits: NonNull::dangling(),
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            let bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            target.bitmap = CreateDIBSection(Some(dc), &bitmap_info, DIB_RGB_COLORS, &mut bits, None, 0)
                .map_err(|_| SubtitleRasterError::Graphics)?;
            target.bits = NonNull::new(bits.cast()).ok_or(SubtitleRasterError::Graphics)?;
            target.bits.as_ptr().write_bytes(0, pixel_count);
            if SelectObject(dc, target.bitmap.into()).is_invalid()
                || SetBkMode(dc, TRANSPARENT) == 0
                || SetTextColor(dc, COLORREF(0x00ff_ffff)).0 == u32::MAX
            {
                return Err(SubtitleRasterError::Graphics);
            }
            Ok(target)
        }
    }

    fn select_font(&mut self, face: PCWSTR, font_px: i32) -> Result<(), SubtitleRasterError> {
        // SAFETY: face is NUL-terminated and the target DC is live on this worker.
        unsafe {
            let font = CreateFontW(
                -font_px,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_TT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                ANTIALIASED_QUALITY,
                0,
                face,
            );
            if font.is_invalid() {
                return Err(SubtitleRasterError::Graphics);
            }
            if SelectObject(self.dc, font.into()).is_invalid() {
                let _ = DeleteObject(font.into());
                return Err(SubtitleRasterError::Graphics);
            }
            if !self.font.is_invalid() {
                let _ = DeleteObject(self.font.into());
            }
            self.font = font;
            Ok(())
        }
    }

    fn selected_family(&self, normalized: &str) -> bool {
        let mut actual = [0; LF_FACESIZE as usize];
        // SAFETY: the selected font and DC are live; the wrapper passes the slice capacity.
        if unsafe { GetTextFaceW(self.dc, Some(&mut actual)) } <= 0 {
            return false;
        }
        let length = actual.iter().position(|&unit| unit == 0).unwrap_or(actual.len());
        String::from_utf16_lossy(&actual[..length]).to_lowercase() == normalized
    }

    fn draw(&self, text: &mut [u16], width: i32, height: i32, position: [f32; 2]) -> Result<(), SubtitleRasterError> {
        let maximum_width = (width * 92 / 100).max(1);
        let mut rect = RECT {
            right: maximum_width,
            bottom: height,
            ..Default::default()
        };
        let flags = DT_CENTER | DT_WORDBREAK | DT_NOPREFIX | DT_EDITCONTROL;
        // SAFETY: the mutable slice is counted, and neither flag requests edits to the text.
        let measured_height = unsafe { DrawTextW(self.dc, text, &mut rect, flags | DT_CALCRECT) };
        if measured_height <= 0 || rect.right <= 0 {
            return Err(SubtitleRasterError::Graphics);
        }
        // The extra pixel preserves the line break after integer width rounding.
        let block_width = (rect.right + 1).clamp(1, width);
        let block_height = measured_height.clamp(1, height);
        let left =
            ((position[0] * width as f32 - block_width as f32 * 0.5).round() as i32).clamp(0, width - block_width);
        let top = ((position[1] * height as f32).round() as i32 - block_height).clamp(0, height - block_height);
        rect = RECT {
            left,
            top,
            right: left + block_width,
            bottom: top + block_height,
        };
        // SAFETY: no GDI operation follows the flush before borrowing CPU coverage bytes.
        if unsafe { DrawTextW(self.dc, text, &mut rect, flags) <= 0 || !GdiFlush().as_bool() } {
            return Err(SubtitleRasterError::Graphics);
        }
        Ok(())
    }

    fn pixels(&self, pixel_count: usize) -> &[u32] {
        // SAFETY: draw has flushed the initialized bitmap. Self keeps that exact storage alive.
        unsafe { std::slice::from_raw_parts(self.bits.as_ptr(), pixel_count) }
    }
}

impl Drop for RasterTarget {
    fn drop(&mut self) {
        // SAFETY: release the private DC before deleting the bitmap and font it selected.
        unsafe {
            let _ = DeleteDC(self.dc);
            if !self.bitmap.is_invalid() {
                let _ = DeleteObject(self.bitmap.into());
            }
            if !self.font.is_invalid() {
                let _ = DeleteObject(self.font.into());
            }
        }
    }
}

fn compose_coverage(mask: &[u32], width: usize) -> Result<Vec<u8>, ImageError> {
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(mask.len() * 4)
        .map_err(|_| ImageError::Capacity)?;
    for (index, &pixel) in mask.iter().enumerate() {
        // Grayscale white ink is drawn onto black, so a single color byte is coverage.
        // Compose a short black drop shadow for contrast on bright video, then unpremultiply
        // and encode once for the normal straight-alpha sRGB image sampling path.
        let ink = f32::from(pixel as u8) / 255.0;
        let shadow = index
            .checked_sub(width)
            .map_or(0.0, |above| f32::from(mask[above] as u8) / 255.0 * 0.75);
        let alpha = ink + shadow * (1.0 - ink);
        if alpha == 0.0 {
            rgba.extend_from_slice(&[0; 4]);
        } else {
            let linear = ink / alpha;
            let encoded = if linear <= 0.003_130_8 {
                linear * 12.92
            } else {
                1.055 * linear.powf(1.0 / 2.4) - 0.055
            };
            let gray = (255.0 * encoded).round() as u8;
            rgba.extend_from_slice(&[gray, gray, gray, (255.0 * alpha).round() as u8]);
        }
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests;
