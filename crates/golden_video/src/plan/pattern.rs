use crate::{LinearColor, TextureId};

/// A checked normalized grid. Width is the distance from each cell boundary on either side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridPattern {
    cells: [u32; 2],
    width: f32,
    foreground: LinearColor,
    background: LinearColor,
}

impl GridPattern {
    /// Admit finite visible lines and 1..4096 cells per axis before shader submission.
    pub fn new(cells: [u32; 2], width: f32, foreground: LinearColor, background: LinearColor) -> Option<Self> {
        (cells.into_iter().all(|count| (1..=4096).contains(&count)) && width.is_finite() && width > 0.0 && width < 0.5)
            .then_some(Self {
                cells,
                width,
                foreground,
                background,
            })
    }
    /// Horizontal and vertical cell counts.
    pub fn cells(self) -> [u32; 2] {
        self.cells
    }
    /// Normalized half-width of each cell boundary line.
    pub fn width(self) -> f32 {
        self.width
    }
    /// Premultiplied linear line color.
    pub fn foreground(self) -> LinearColor {
        self.foreground
    }
    /// Premultiplied linear cell color.
    pub fn background(self) -> LinearColor {
        self.background
    }
}

/// Generic GPU-generated image content; apps choose calibration policy and palettes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pattern {
    /// A repeated normalized grid.
    Grid(GridPattern),
    /// Eight ordered vertical stripes covering the unit rectangle.
    Bars([LinearColor; 8]),
}

/// Texture sampling or source-independent procedural content, composed by the same layer path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayerSource {
    /// A dependency on another render pass or external input.
    Texture(TextureId),
    /// A bounded procedural draw with no texture/input demand.
    Pattern(Pattern),
}

impl LayerSource {
    /// The actual texture dependency, excluding procedural content.
    pub fn texture(self) -> Option<TextureId> {
        match self {
            Self::Texture(texture) => Some(texture),
            Self::Pattern(_) => None,
        }
    }
}

impl From<TextureId> for LayerSource {
    fn from(texture: TextureId) -> Self {
        Self::Texture(texture)
    }
}
