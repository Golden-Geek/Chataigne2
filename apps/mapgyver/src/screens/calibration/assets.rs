use golden_video::{
    AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor,
    image::{FontAtlas, ImageAtlas, ImageData},
    plan::UnitRect,
};
use std::{
    io::Cursor,
    sync::{Arc, Weak},
};

/// Preparation-owner memoization only. Live plans/textures own the pixels, not this cache.
#[derive(Default)]
pub(crate) struct CalibrationAssets {
    logo: Weak<ImageData>,
    atlases: Vec<([u16; 2], Weak<ImageData>, AtlasLayout)>,
}

#[derive(Clone, Copy)]
pub(super) struct AtlasLayout {
    pub logo: UnitRect,
    pub readout: UnitRect,
    pub labels: UnitRect,
    pub readout_cell: [u32; 2],
    pub label_cell: [u32; 2],
}

pub(super) struct CalibrationAtlas {
    pub image: Arc<ImageData>,
    pub layout: AtlasLayout,
}
impl CalibrationAssets {
    pub(super) fn atlas(&mut self, extent: Extent2D) -> Result<CalibrationAtlas, String> {
        let diameter = extent.width().min(extent.height()) as f32;
        let cell_pixels = super::layout(extent).cell_pixels;
        let key = [
            (diameter * 0.065).ceil().clamp(8., 192.) as u16,
            (cell_pixels * 0.18).ceil().clamp(4., 64.) as u16,
        ];
        self.atlases.retain(|(_, image, _)| image.strong_count() != 0);
        if let Some((_, image, layout)) = self.atlases.iter().find(|(size, _, _)| *size == key)
            && let Some(image) = image.upgrade()
        {
            return Ok(CalibrationAtlas { image, layout: *layout });
        }
        // This owner is driven on the channel preparation worker. Outlines are rasterized
        // only for changed destination densities; live clock/FPS values reuse these glyphs.
        let font = include_bytes!("../../../assets/fonts/NotoSansMono-Regular.ttf");
        let readout = FontAtlas::rasterize(font, key[0])?;
        let labels = FontAtlas::rasterize(font, key[1])?;
        let logo = self.logo()?;
        let atlas = ImageAtlas::pack(&[logo.as_ref(), readout.image(), labels.image()])?;
        let layout = AtlasLayout {
            logo: atlas.region(0).unwrap(),
            readout: atlas.region(1).unwrap(),
            labels: atlas.region(2).unwrap(),
            readout_cell: readout.cell(),
            label_cell: labels.cell(),
        };
        let image = atlas.image().clone();
        // Weak metadata cannot retain a closed plan's pixels. Bound even the lookup list.
        if self.atlases.len() < 8 {
            self.atlases.push((key, Arc::downgrade(&image), layout));
        }
        Ok(CalibrationAtlas { image, layout })
    }
    pub(super) fn logo(&mut self) -> Result<Arc<ImageData>, String> {
        if let Some(image) = self.logo.upgrade() {
            return Ok(image);
        }
        let mut decoder = png::Decoder::new_with_limits(
            Cursor::new(include_bytes!("../../../assets/mapgyver-icon.png")),
            png::Limits { bytes: 2 * 1024 * 1024 },
        );
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        decoder.set_ignore_text_chunk(true);
        let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
        let info = reader.info();
        if info.width == 0 || info.height == 0 || info.width > 512 || info.height > 512 {
            return Err("Calibration bitmap exceeds its fixed 512x512 decode bound".into());
        }
        let bytes = reader.output_buffer_size().ok_or("Calibration bitmap size overflow")?;
        if bytes > 512 * 512 * 4 {
            return Err("Calibration bitmap exceeds its fixed pixel budget".into());
        }
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(bytes)
            .map_err(|_| "Calibration bitmap allocation is unavailable")?;
        pixels.resize(bytes, 0);
        let output = reader.next_frame(&mut pixels).map_err(|error| error.to_string())?;
        if output.color_type != png::ColorType::Rgba || output.bit_depth != png::BitDepth::Eight {
            return Err("Generated calibration bitmap must contain RGBA8 pixels".into());
        }
        pixels.truncate(output.buffer_size());
        let image = Arc::new(
            ImageData::new(
                TextureDescriptor {
                    extent: Extent2D::new(output.width, output.height).map_err(|error| error.to_string())?,
                    format: PixelFormat::Rgba8Unorm,
                    color_encoding: ColorEncoding::Srgb,
                    alpha_mode: AlphaMode::Straight,
                },
                pixels,
            )
            .map_err(|error| error.to_string())?,
        );
        self.logo = Arc::downgrade(&image);
        Ok(image)
    }
}
